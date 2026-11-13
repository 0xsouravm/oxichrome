use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::ItemFn;

use crate::codegen::Order;
use crate::i18n::Text;
use crate::metadata;

#[derive(Default)]
struct Args {
    title: Option<Text>,
    styles: Vec<crate::codegen::page::Style>,
    open_on_action_click: bool,
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
                "style" => crate::codegen::page::parse_styles(input, &mut args.styles, &key)?,
                "open_on_action_click" => {
                    if args.open_on_action_click {
                        return Err(syn::Error::new(key.span(), "`open_on_action_click` is set twice"));
                    }
                    if input.peek(syn::Token![=]) {
                        return Err(syn::Error::new(key.span(), "write `open_on_action_click` on its own, without `= ...`"));
                    }
                    args.open_on_action_click = true;
                }
                "path" | "default_path" | "default_panel" | "panel" => {
                    return Err(syn::Error::new(
                        key.span(),
                        "the panel page comes from this component; oxichrome writes the HTML for you, so remove it",
                    ))
                }
                "open_at_install" => {
                    return Err(syn::Error::new(
                        key.span(),
                        "Chrome never opens a side panel at install, so oxichrome keeps Firefox the same; open it from a click with `open_on_action_click` or `side_panel::open_on_click`",
                    ))
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown side panel option `{other}`; use `title = \"...\"`, `open_on_action_click` or `style = \"file.css\"`"),
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
    let args: Args = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;
    if let Some(input) = func.sig.inputs.first() {
        return Err(syn::Error::new_spanned(input, "a side panel component takes no arguments"));
    }
    if func.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(func.sig.asyncness, "a side panel component can't be `async`; load data inside it with a resource or `spawn_local`"));
    }

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;
    let ret = &func.sig.output;

    let at = format!("{}:{}:{}", order.file, order.line, order.column);
    let checks = args.title.as_ref().map(|title| title.checks(&at)).unwrap_or_default();
    let style_checks = crate::codegen::page::check_files(&args.styles)?;
    let entry = metadata::emit(serde_json::json!({
        "kind": "side_panel",
        "fn_name": fn_name.to_string(),
        "title": args.title.as_ref().map(Text::value),
        "open_on_action_click": args.open_on_action_click,
        "styles": crate::codegen::page::paths(&args.styles),
    }));

    Ok(quote! {
        #entry
        #checks
        #style_checks

        #(#attrs)*
        #[::leptos::component]
        #vis fn #fn_name() #ret #fn_body

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(js_name = __oxichrome_mount_side_panel, wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn __oxichrome_only_one_side_panel_per_extension() {
            let handle = ::leptos::mount::mount_to_body(#fn_name);
            std::mem::forget(handle);
        }
    })
}
