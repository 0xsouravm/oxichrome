use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr};

use crate::metadata;
use crate::parse::ContentScriptArgs;

pub fn expand(attr: TokenStream, item: TokenStream, order: crate::codegen::Order) -> Result<TokenStream, syn::Error> {
    let args: ContentScriptArgs = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[oxichrome::content_script] function must be async",
        ));
    }

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;

    let wrapper_name = crate::codegen::export_name("cs", fn_name, &order);

    let run_at = match &args.run_at {
        None => None,
        Some(variant) => Some(match variant.to_string().as_str() {
            "DocumentStart" => "document_start",
            "DocumentEnd" => "document_end",
            "DocumentIdle" => "document_idle",
            other => {
                return Err(syn::Error::new(
                    variant.span(),
                    format!("unknown run_at `{other}`; expected `DocumentStart`, `DocumentEnd` or `DocumentIdle`"),
                ))
            }
        }),
    };
    let entry = metadata::emit(serde_json::json!({
        "kind": "content_script",
        "fn_name": fn_name.to_string(),
        "export": wrapper_name.to_string(),
        "matches": args.matches.iter().map(LitStr::value).collect::<Vec<_>>(),
        "run_at": run_at,
        "all_frames": args.all_frames,
        "css": args.css.iter().map(LitStr::value).collect::<Vec<_>>(),
    }));

    let run_at_check = args.run_at.map(|variant| {
        quote! {
            #[doc(hidden)]
            const _: oxichrome::RunAt = oxichrome::RunAt::#variant;
        }
    });

    let css_checks: Vec<_> = args.css.iter().map(|css_file| {
        let path = format!("static/{}", css_file.value());
        quote! {
            #[doc(hidden)]
            const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #path));
        }
    }).collect();

    Ok(quote! {
        #entry
        #run_at_check
        #(#css_checks)*

        #(#attrs)*
        #vis async fn #fn_name() #fn_body

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn #wrapper_name() {
            oxichrome::__private::wasm_bindgen_futures::spawn_local(async {
                #fn_name().await;
            });
        }
    })
}
