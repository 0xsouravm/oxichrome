use proc_macro2::TokenStream;
use quote::quote;

use crate::metadata;
use syn::ItemFn;

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let styles = crate::codegen::page::parse_page_args(attr, "options page")?;
    let func: ItemFn = syn::parse2(item)?;
    let style_checks = crate::codegen::page::check_files(&styles)?;

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;
    let sig = &func.sig;
    let ret = &sig.output;

    let entry = metadata::emit(serde_json::json!({
        "kind": "options_page",
        "fn_name": fn_name.to_string(),
        "styles": crate::codegen::page::paths(&styles),
    }));

    Ok(quote! {
        #entry
        #style_checks

        #(#attrs)*
        #[::leptos::component]
        #vis fn #fn_name() #ret #fn_body

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(js_name = __oxichrome_mount_options)]
        pub fn __oxichrome_only_one_options_page_per_extension() {
            let handle = ::leptos::mount::mount_to_body(#fn_name);
            std::mem::forget(handle);
        }
    })
}
