use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

use crate::parse::ContentScriptArgs;

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: ContentScriptArgs = syn::parse2(attr)?;
    let _matches = args.matches;
    let _all_frames = args.all_frames;
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

    let wrapper_name = syn::Ident::new(
        &format!("__oxichrome_cs_{fn_name}"),
        fn_name.span(),
    );

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
