use proc_macro2::TokenStream;
use quote::quote;

use crate::metadata;
use syn::ItemFn;

pub fn expand(item: TokenStream, order: crate::codegen::Order) -> Result<TokenStream, syn::Error> {
    let func: ItemFn = syn::parse2(item)?;

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[oxichrome::background] function must be async",
        ));
    }

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;

    let wrapper_name = crate::codegen::export_name("bg", fn_name, &order);

    let entry = metadata::emit(serde_json::json!({
        "kind": "background",
        "fn_name": fn_name.to_string(),
        "export": wrapper_name.to_string(),
    }));

    Ok(quote! {
        #entry

        #(#attrs)*
        #vis async fn #fn_name() #fn_body

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn #wrapper_name() {
            oxichrome::__private::wasm_bindgen_futures::spawn_local(async {
                #fn_name().await;
            });
        }
    })
}
