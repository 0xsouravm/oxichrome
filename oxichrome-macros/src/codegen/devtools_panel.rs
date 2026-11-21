use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{ItemFn, LitStr};

use crate::codegen::Order;
use crate::i18n::Text;
use crate::metadata;

#[derive(Default)]
struct Args {
    title: Option<Text>,
    icon: Option<(String, proc_macro2::Span)>,
    styles: Vec<crate::codegen::page::Style>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Args::default();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            match key.to_string().as_str() {
                "title" => {
                    if args.title.is_some() {
                        return Err(syn::Error::new(key.span(), "`title` is set twice"));
                    }
                    input.parse::<syn::Token![=]>()?;
                    args.title = Some(Text::parse_field(input, "title")?);
                }
                "icon" => {
                    if args.icon.is_some() {
                        return Err(syn::Error::new(key.span(), "`icon` is set twice"));
                    }
                    input.parse::<syn::Token![=]>()?;
                    let path: LitStr = input.parse().map_err(|err| syn::Error::new(err.span(), "`icon` takes a PNG file in `static/`, like \"icons/panel.png\""))?;
                    args.icon = Some((crate::parse::normalize_icon_path(&path)?, path.span()));
                }
                "style" => crate::codegen::page::parse_styles(input, &mut args.styles, &key)?,
                "page" | "path" | "devtools_page" => {
                    return Err(syn::Error::new(
                        key.span(),
                        "the panel page comes from this component; oxichrome writes the HTML and the devtools page for you, so remove it",
                    ))
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown devtools panel option `{other}`; use `title = \"...\"`, `icon = \"file.png\"` or `style = \"file.css\"`"),
                    ))
                }
            }
            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(args)
    }
}

pub fn expand(attr: TokenStream, item: TokenStream, order: Order) -> Result<TokenStream, syn::Error> {
    let attr_span = proc_macro2::Span::call_site();
    let args: Args = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;
    if let Some(input) = func.sig.inputs.first() {
        return Err(syn::Error::new_spanned(input, "a devtools panel component takes no arguments"));
    }
    if func.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(func.sig.asyncness, "a devtools panel component can't be `async`; load data inside it with a resource or `spawn_local`"));
    }
    let Some(title) = args.title.as_ref() else {
        return Err(syn::Error::new(attr_span, "a devtools panel needs a `title`, which both browsers show on its tab: `#[oxichrome::devtools_panel(title = \"My panel\")]`"));
    };
    if let Text::Literal(text) = title {
        if text.value().trim().is_empty() {
            return Err(syn::Error::new(attr_span, "the devtools panel `title` is empty; both browsers show it on the panel's tab"));
        }
    }

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;
    let ret = &func.sig.output;

    let at = format!("{}:{}:{}", order.file, order.line, order.column);
    let checks = title.checks(&at);
    let style_checks = crate::codegen::page::check_files(&args.styles)?;
    let icon_check = match &args.icon {
        Some((path, span)) => {
            let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
            let file = std::path::Path::new(&root).join("static").join(path);
            if !file.is_file() {
                return Err(syn::Error::new(*span, format!("there's no `static/{path}`; add it, or fix the path")));
            }
            let file = file.to_string_lossy().into_owned();
            quote! {
                #[doc(hidden)]
                const _: &[u8] = include_bytes!(#file);
            }
        }
        None => TokenStream::new(),
    };
    let mount = format_ident!("__oxichrome_mount_devtools_{}", fn_name);
    let entry = metadata::emit(serde_json::json!({
        "kind": "devtools_panel",
        "fn_name": fn_name.to_string(),
        "export": mount.to_string(),
        "title": title.value(),
        "icon": args.icon.as_ref().map(|(path, _)| path),
        "styles": crate::codegen::page::paths(&args.styles),
        "file": order.file,
        "line": order.line,
        "column": order.column,
    }));

    Ok(quote! {
        #entry
        #checks
        #style_checks
        #icon_check

        #(#attrs)*
        #[::leptos::component]
        #vis fn #fn_name() #ret #fn_body

        #[doc(hidden)]
        #[allow(non_snake_case)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(js_name = #mount, wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn #mount() {
            let handle = ::leptos::mount::mount_to_body(#fn_name);
            std::mem::forget(handle);
        }
    })
}
