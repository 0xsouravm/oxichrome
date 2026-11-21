use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, ItemFn};

use crate::metadata;
use crate::parse::EventArgs;

pub fn expand(attr: TokenStream, item: TokenStream, order: crate::codegen::Order) -> Result<TokenStream, syn::Error> {
    let args: EventArgs = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;

    if args.namespace == "context_menus" {
        return Err(syn::Error::new_spanned(
            &args.namespace,
            "context menu clicks go to each item's own handler: put `#[oxichrome::menu(title = \"...\")]` on an async fn instead",
        ));
    }

    if args.namespace == "commands" {
        return Err(syn::Error::new_spanned(
            &args.namespace,
            "each keyboard command has its own handler: put `#[oxichrome::command(description = \"...\")]` on an async fn instead",
        ));
    }

    if args.namespace == "bookmarks" && ["on_children_reordered", "on_import_began", "on_import_ended"].iter().any(|name| args.event_name == name) {
        return Err(syn::Error::new_spanned(
            &args.event_name,
            format!(
                "`bookmarks::{}` only exists in Chrome, so oxichrome leaves it out; `on_created`, `on_changed`, `on_moved` and `on_removed` work in both browsers",
                args.event_name
            ),
        ));
    }

    if args.namespace == "history" && args.event_name == "on_title_changed" {
        return Err(syn::Error::new_spanned(
            &args.event_name,
            "`history::on_title_changed` only exists in Firefox, so oxichrome leaves it out; `on_visited` and `on_visit_removed` work in both browsers",
        ));
    }

    match crate::events::check(&args.namespace.to_string(), &args.event_name.to_string()) {
        Ok(()) => {}
        Err(crate::events::Unknown::Namespace(message)) => return Err(syn::Error::new_spanned(&args.namespace, message)),
        Err(crate::events::Unknown::Event(message)) => return Err(syn::Error::new_spanned(&args.event_name, message)),
    }

    if let Some(filter) = &args.filter {
        if args.namespace != "web_request" {
            return Err(syn::Error::new(filter.span, format!("only `web_request` events take a filter; `{}::{}` takes just the event", args.namespace, args.event_name)));
        }
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

    let export_fn_name = crate::codegen::export_name("on", fn_name, &order);
    let check_fn_name = format_ident!("__oxichrome_check_{}", fn_name);

    let namespace = &args.namespace;
    let event_name = &args.event_name;

    let bridge_fn_name = format_ident!("chrome_{namespace}_{event_name}_add_listener");

    let js_type = quote! { oxichrome::__private::wasm_bindgen::JsValue };

    let is_message = namespace == "runtime" && event_name == "on_message";
    let replies = is_message
        && match &func.sig.output {
            syn::ReturnType::Default => false,
            syn::ReturnType::Type(_, ty) => !matches!(ty.as_ref(), syn::Type::Tuple(tuple) if tuple.elems.is_empty()),
        };
    let is_auth = namespace == "web_request" && event_name == "on_auth_required";
    let has_return = !matches!(&func.sig.output, syn::ReturnType::Default)
        && !matches!(&func.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Tuple(tuple) if tuple.elems.is_empty()));
    if is_auth && !has_return {
        return Err(syn::Error::new_spanned(
            &func.sig.ident,
            "an `on_auth_required` handler answers the challenge: return a `web_request::Auth` (`Auth::credentials(user, password)`, `Auth::Cancel` or `Auth::Prompt`)",
        ));
    }
    let raw_count = if replies { 3 } else { arg_types.len() };
    let raw_args: Vec<_> = (0..raw_count)
        .map(|i| format_ident!("__oxichrome_arg{}", i))
        .collect();
    let call_args = &raw_args[..arg_types.len()];

    let conversions = raw_args
        .iter()
        .zip(&arg_types)
        .enumerate()
        .map(|(i, (raw, ty))| {
            let position = i + 1;
            let handler = fn_name.to_string();
            if is_message && position == 1 {
                return quote! {
                    let #raw: #ty = {
                        #[allow(unused_imports)]
                        use oxichrome::core::runtime::{__MessageAsJs as _, __MessageAsSerde as _};
                        match (&oxichrome::core::runtime::__MessageProbe::<#ty>(::core::marker::PhantomData)).__read(#raw) {
                            ::core::option::Option::Some(message) => message,
                            ::core::option::Option::None => return Ok(#js_type::UNDEFINED),
                        }
                    };
                };
            }
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
    let is_js_value = |ty: &syn::Type| matches!(ty, syn::Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "JsValue"));
    let message_check = match arg_types.first() {
        Some(ty) if is_message && !is_js_value(ty) => quote_spanned! {ty.span()=>
            oxichrome::core::runtime::__assert_message::<#ty>();
        },
        _ => quote! {},
    };
    let arg_checks = arg_types.iter().enumerate().filter(|(i, _)| !(is_message && *i == 0)).map(|(i, ty)| {
        let position = i + 1;
        quote_spanned! {ty.span()=>
            oxichrome::core::event::assert_arg::<#ty, oxichrome::core::event::events::#marker, #position>();
        }
    });

    let arg_count = arg_types.len();
    let too_many = format!(
        "`{fn_name}` takes {arg_count} arguments, more than `{namespace}::{event_name}` passes; remove the extra ones"
    );

    let keep_reply = if replies {
        quote! { let __oxichrome_reply = __oxichrome_arg2.clone(); }
    } else {
        quote! {}
    };
    let call = if is_auth {
        let output = match &func.sig.output {
            syn::ReturnType::Type(_, ty) => ty.as_ref().clone(),
            syn::ReturnType::Default => unreachable!(),
        };
        quote_spanned! {output.span()=>
            let __oxichrome_value: oxichrome::core::web_request::Auth = #fn_name(#(#call_args),*).await;
        }
    } else if replies {
        quote! {
            let __oxichrome_value = #fn_name(#(#call_args),*).await;
            oxichrome::core::runtime::__reply(&__oxichrome_reply, &__oxichrome_value)?;
        }
    } else {
        quote! { #fn_name(#(#call_args),*).await; }
    };

    let mut entry = serde_json::json!({
        "kind": "event",
        "fn_name": fn_name.to_string(),
        "export": export_fn_name.to_string(),
        "namespace": namespace.to_string(),
        "event_name": event_name.to_string(),
    });
    if namespace == "web_request" {
        let filter = args.filter.as_ref();
        let urls: Vec<String> = filter.map(|f| f.urls.iter().map(|url| url.value()).collect()).filter(|urls: &Vec<String>| !urls.is_empty()).unwrap_or_else(|| vec!["<all_urls>".to_string()]);
        let types: Vec<String> = filter.map(|f| f.types.clone()).unwrap_or_default();
        entry["filter"] = serde_json::json!({ "urls": urls, "types": types });
    }
    let entry = metadata::emit(entry);
    let tail = if is_auth {
        quote! { oxichrome::core::web_request::__auth_answer(&__oxichrome_value) }
    } else {
        quote! { Ok(#js_type::UNDEFINED) }
    };

    Ok(quote! {
        #func

        #entry

        #[doc(hidden)]
        #[allow(dead_code)]
        fn #check_fn_name() {
            let _ = oxichrome::core::js_bridge::#bridge_fn_name;
            #message_check
            #(#arg_checks)*
        }

        #[doc(hidden)]
        const _: () = assert!(
            #arg_count <= <oxichrome::core::event::events::#marker as oxichrome::core::event::Event>::ARGS,
            #too_many
        );

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn #export_fn_name(#(#raw_args: #js_type),*) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                #keep_reply
                #(#conversions)*
                #call
                #tail
            })
        }
    })
}
