use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, ItemFn};

use crate::metadata;
use crate::parse::EventArgs;

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: EventArgs = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;

    if args.namespace == "context_menus" {
        return Err(syn::Error::new_spanned(
            &args.namespace,
            "context menu clicks go to each item's own handler: put `#[oxichrome::menu(title = \"...\")]` on an async fn instead",
        ));
    }

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[oxichrome::on(...)] function must be async",
        ));
    }

    let fn_name = &func.sig.ident;

    let mut arg_types = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pat_type) => arg_types.push(pat_type.ty.as_ref().clone()),
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "#[oxichrome::on(...)] function can't take `self`",
                ))
            }
        }
    }

    let export_fn_name = format_ident!("__oxichrome_on_{}", fn_name);
    let check_fn_name = format_ident!("__oxichrome_check_{}", fn_name);

    let namespace = &args.namespace;
    let event_name = &args.event_name;

    let bridge_fn_name = format_ident!("chrome_{namespace}_{event_name}_add_listener");

    let js_type = quote! { oxichrome::__private::wasm_bindgen::JsValue };

    let raw_args: Vec<_> = (0..arg_types.len())
        .map(|i| format_ident!("__oxichrome_arg{}", i))
        .collect();

    let conversions = raw_args
        .iter()
        .zip(&arg_types)
        .enumerate()
        .map(|(i, (raw, ty))| {
            let position = i + 1;
            let handler = fn_name.to_string();
            quote! {
                let #raw = <#ty as oxichrome::core::event::FromEventArg>::from_event_arg(#raw)
                    .map_err(|err| {
                        #js_type::from(oxichrome::__private::js_sys::Error::new(&format!(
                            "[oxichrome] {} argument {} could not be read: {}",
                            #handler, #position, err
                        )))
                    })?;
            }
        });

    let fnmut_params = raw_args.iter().map(|_| quote! { #js_type });

    let camel = |ident: &syn::Ident| {
        ident
            .to_string()
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<String>()
    };
    let marker = format_ident!("{}{}", camel(namespace), camel(event_name));
    let arg_checks = arg_types.iter().enumerate().map(|(i, ty)| {
        let position = i + 1;
        quote_spanned! {ty.span()=>
            oxichrome::core::event::assert_arg::<#ty, oxichrome::core::event::events::#marker, #position>();
        }
    });

    let entry = metadata::emit(serde_json::json!({
        "kind": "event",
        "fn_name": fn_name.to_string(),
        "namespace": namespace.to_string(),
        "event_name": event_name.to_string(),
    }));

    Ok(quote! {
        #func

        #entry

        #[doc(hidden)]
        #[allow(dead_code)]
        fn #check_fn_name() {
            use oxichrome::__private::wasm_bindgen::prelude::Closure;
            let _: fn(&Closure<dyn FnMut(#(#fnmut_params),*)>) =
                oxichrome::core::js_bridge::#bridge_fn_name;
            #(#arg_checks)*
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn #export_fn_name(#(#raw_args: #js_type),*) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                #(#conversions)*
                #fn_name(#(#raw_args),*).await;
                Ok(#js_type::UNDEFINED)
            })
        }
    })
}
