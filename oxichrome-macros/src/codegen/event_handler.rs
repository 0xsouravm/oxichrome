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

    let export_fn_name = format_ident!("__oxichrome_on_{}", fn_name);
    let check_fn_name = format_ident!("__oxichrome_check_{}", fn_name);

    let namespace = &args.namespace;
    let event_name = &args.event_name;

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

    let export_params = param_names.iter().map(|name| {
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
        fn #check_fn_name() {
            use oxichrome::__private::wasm_bindgen::prelude::Closure;
            let _: fn(&Closure<dyn FnMut(#(#fnmut_params),*)>) =
                oxichrome::core::js_bridge::#bridge_fn_name;
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn #export_fn_name(#(#export_params),*) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                #fn_name(#(#forward_args),*).await;
                Ok(#js_type::UNDEFINED)
            })
        }
    })
}
