use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat};

use crate::parse::EventArgs;

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: EventArgs = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[oxichrome::on(...)] function must be async",
        ));
    }

    let fn_name = &func.sig.ident;
    let fn_body = &func.block;
    let vis = &func.vis;
    let attrs = &func.attrs;

    let register_fn_name = format_ident!("__oxichrome_register_{}", fn_name);

    let namespace = &args.namespace;
    let event_name = &args.event_name;

    // runtime::on_installed -> chrome_runtime_on_installed_add_listener
    let bridge_fn_name = format_ident!("chrome_{namespace}_{event_name}_add_listener");

    let param_names: Vec<_> = func
        .sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => format_ident!("_arg"),
            },
            FnArg::Receiver(_) => format_ident!("self_"),
        })
        .collect();

    let js_type = quote! { oxichrome::__private::wasm_bindgen::JsValue };

    let fn_params = param_names.iter().map(|name| {
        quote! { #name: #js_type }
    });

    let closure_params = param_names.iter().map(|name| {
        quote! { #name: #js_type }
    });

    let forward_args = param_names.iter().map(|name| {
        quote! { #name }
    });

    let fnmut_params = param_names.iter().map(|_| {
        quote! { #js_type }
    });

    Ok(quote! {
        #(#attrs)*
        #vis async fn #fn_name(#(#fn_params),*) #fn_body

        #[doc(hidden)]
        #[allow(dead_code)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn #register_fn_name() {
            use oxichrome::__private::wasm_bindgen::prelude::*;
            use oxichrome::__private::wasm_bindgen::JsCast;

            let closure = Closure::wrap(Box::new(move |#(#closure_params),*| {
                oxichrome::__private::wasm_bindgen_futures::spawn_local(async move {
                    #fn_name(#(#forward_args),*).await;
                });
            }) as Box<dyn FnMut(#(#fnmut_params),*)>);

            oxichrome::core::js_bridge::#bridge_fn_name(&closure);
            closure.forget();
        }
    })
}
