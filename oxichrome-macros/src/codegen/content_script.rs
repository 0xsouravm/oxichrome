use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
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

    if !func.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig.generics,
            "#[oxichrome::content_script] function can't be generic",
        ));
    }
    if let Some(receiver) = func.sig.receiver() {
        return Err(syn::Error::new_spanned(
            receiver,
            "#[oxichrome::content_script] function can't take `self`",
        ));
    }
    if args.on_demand {
        return on_demand(func, order);
    }
    if !func.sig.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "a content script with `matches` starts by itself and gets no argument; remove `matches` to run it on demand with `SCRIPT.run(tab_id, arg)`",
        ));
    }
    if let syn::ReturnType::Type(_, ty) = &func.sig.output {
        return Err(syn::Error::new_spanned(
            ty,
            "a content script with `matches` starts by itself and nobody receives its result; remove `matches` to run it on demand and get the value back from `SCRIPT.run`",
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

fn borrows(ty: &syn::Type) -> bool {
    fn scan(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Punct(punct) => punct.as_char() == '&' || punct.as_char() == '\'',
            proc_macro2::TokenTree::Group(group) => scan(group.stream()),
            _ => false,
        })
    }
    scan(quote! { #ty })
}

fn on_demand(mut func: ItemFn, order: crate::codegen::Order) -> Result<TokenStream, syn::Error> {
    let fn_name = func.sig.ident.clone();
    let fn_name = &fn_name;
    let name = fn_name.to_string();
    let vis = func.vis.clone();
    let export = crate::codegen::export_name("script", fn_name, &order);

    let inputs: Vec<&syn::PatType> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => Some(pat_type),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    if inputs.len() > 1 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "an on-demand script takes at most one argument; put several values in a struct or a tuple",
        ));
    }
    for pat_type in &inputs {
        if borrows(&pat_type.ty) {
            return Err(syn::Error::new_spanned(
                &pat_type.ty,
                "an on-demand script gets a copy of its argument inside the page, so it can't borrow; take an owned type like `String` or `Vec<T>`",
            ));
        }
    }
    if let syn::ReturnType::Type(_, ty) = &func.sig.output {
        if borrows(ty) {
            return Err(syn::Error::new_spanned(
                ty,
                "an on-demand script sends a copy of its result out of the page, so it can't return a borrow; return an owned type like `String`",
            ));
        }
    }
    let body_name = quote::format_ident!("{}_body", export);
    let (arg_ty, call) = match inputs.first() {
        None => (quote! { () }, quote! { |_: ()| #body_name() }),
        Some(pat_type) => {
            let ty = &pat_type.ty;
            (quote! { #ty }, quote! { #body_name })
        }
    };
    let ret_ty = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => {
            if let syn::Type::ImplTrait(_) = ty.as_ref() {
                return Err(syn::Error::new_spanned(
                    ty,
                    "an on-demand script must name its return type; `impl Trait` can't be sent back to the caller",
                ));
            }
            quote! { #ty }
        }
    };
    let checks = {
        let arg_check = inputs.first().map(|pat_type| {
            let ty = &pat_type.ty;
            quote_spanned! {ty.span()=>
                const _: fn() = || {
                    fn arg<T: oxichrome::__private::serde::Serialize + oxichrome::__private::serde::de::DeserializeOwned>() {}
                    arg::<#ty>();
                };
            }
        });
        let ret_check = match &func.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(quote_spanned! {ty.span()=>
                const _: fn() = || {
                    fn result<T: oxichrome::__private::serde::Serialize + oxichrome::__private::serde::de::DeserializeOwned>() {}
                    result::<#ty>();
                };
            }),
        };
        quote! { #arg_check #ret_check }
    };

    let entry = metadata::emit(serde_json::json!({
        "kind": "script",
        "name": name,
        "file": order.file,
        "line": order.line,
        "column": order.column,
    }));
    let export_str = export.to_string();
    func.sig.ident = body_name.clone();
    func.vis = syn::Visibility::Inherited;
    func.attrs.push(syn::parse_quote!(#[doc(hidden)]));
    let js = quote! { oxichrome::__private::wasm_bindgen::JsValue };

    Ok(quote! {
        #entry
        #checks

        #func

        #[allow(non_snake_case)]
        #vis mod #fn_name {
            #[allow(unused_imports)]
            use super::*;
            pub const SCRIPT: oxichrome::scripting::Script<#arg_ty, #ret_ty> =
                oxichrome::scripting::Script::__new(#name, #export_str, env!("CARGO_PKG_NAME"));
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn #export(arg: #js) -> ::core::result::Result<#js, #js> {
            oxichrome::scripting::__run_export::<#arg_ty, #ret_ty, _, _>(#name, arg, #call).await
        }
    })
}
