use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemFn;

pub fn expand(item: TokenStream) -> Result<TokenStream, syn::Error> {
    let func: ItemFn = syn::parse2(item)?;

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;
    let sig = &func.sig;
    let ret = &sig.output;

    Ok(quote! {
        #(#attrs)*
        #[::leptos::component]
        #vis fn #fn_name() #ret #fn_body

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn __oxichrome_mount_options() {
            let handle = ::leptos::mount::mount_to_body(#fn_name);
            std::mem::forget(handle);
        }
    })
}
