use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, ItemFn};

use crate::codegen::{export_name, Order};
use crate::metadata;
use crate::parse::parse_command_args;

pub fn expand(attr: TokenStream, item: TokenStream, order: Order) -> syn::Result<TokenStream> {
    let args = syn::parse::Parser::parse2(parse_command_args, attr)?;
    let func: ItemFn = syn::parse2(item)?;

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "#[oxichrome::command(...)] function must be async",
        ));
    }
    if !func.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig.generics,
            "#[oxichrome::command(...)] function can't be generic",
        ));
    }
    let fn_name = &func.sig.ident;
    let name = fn_name.to_string();
    if name.starts_with("_execute") {
        return Err(syn::Error::new_spanned(
            fn_name,
            "names starting with `_execute` are reserved by the browser; for the toolbar button use `action = { shortcut = \"...\" }` in `#[oxichrome::extension]`",
        ));
    }
    let mut arg_types = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pat_type) => arg_types.push(pat_type.ty.as_ref().clone()),
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "#[oxichrome::command(...)] function can't take `self`",
                ))
            }
        }
    }
    if arg_types.len() > 1 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "a command handler takes no arguments or one `commands::Press`",
        ));
    }

    let vis = &func.vis;
    let export = export_name("command", fn_name, &order);
    let checks = args.description.checks(&format!("{}:{}:{}", order.file, order.line, order.column));
    let entry = metadata::emit(serde_json::json!({
        "kind": "command",
        "name": name,
        "export": export.to_string(),
        "description": args.description.value(),
        "shortcut": args.shortcut.as_ref().map(|shortcut| shortcut.to_json()),
        "file": order.file,
        "line": order.line,
        "column": order.column,
    }));

    let js_type = quote! { oxichrome::__private::wasm_bindgen::JsValue };
    let call = match arg_types.first() {
        None => quote! { #fn_name().await; },
        Some(ty) => {
            let check = quote_spanned! {ty.span()=>
                oxichrome::core::event::assert_arg::<#ty, oxichrome::core::event::events::CommandsOnCommand, 1>();
            };
            quote! {
                #check
                let __oxichrome_press = <#ty as oxichrome::core::event::FromEventArg>::from_event_arg(__oxichrome_press)
                    .map_err(|err| {
                        #js_type::from(oxichrome::__private::js_sys::Error::new(&format!(
                            "[oxichrome] {} argument 1 could not be read: {}",
                            #name, err
                        )))
                    })?;
                #fn_name(__oxichrome_press).await;
            }
        }
    };

    Ok(quote! {
        #func

        #entry
        #checks

        #[doc(hidden)]
        #vis mod #fn_name {
            pub const COMMAND: oxichrome::commands::Command =
                oxichrome::commands::Command::__new(#name);
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn #export(__oxichrome_press: #js_type) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                let _ = &__oxichrome_press;
                #call
                Ok(#js_type::UNDEFINED)
            })
        }
    })
}
