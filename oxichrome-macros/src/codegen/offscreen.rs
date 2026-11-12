use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, ItemFn, LitStr, Token};

use crate::codegen::content_script::borrows;
use crate::metadata;

const REASONS: &[(&str, &str)] = &[
    ("AudioPlayback", "AUDIO_PLAYBACK"),
    ("BatteryStatus", "BATTERY_STATUS"),
    ("Blobs", "BLOBS"),
    ("Clipboard", "CLIPBOARD"),
    ("DisplayMedia", "DISPLAY_MEDIA"),
    ("DomParser", "DOM_PARSER"),
    ("DomScraping", "DOM_SCRAPING"),
    ("Geolocation", "GEOLOCATION"),
    ("IframeScripting", "IFRAME_SCRIPTING"),
    ("LocalStorage", "LOCAL_STORAGE"),
    ("MatchMedia", "MATCH_MEDIA"),
    ("UserMedia", "USER_MEDIA"),
    ("WebRtc", "WEB_RTC"),
    ("Workers", "WORKERS"),
];

struct Args {
    reasons: Vec<Ident>,
    justification: LitStr,
}

fn reason_list() -> String {
    REASONS.iter().map(|(ident, _)| format!("`{ident}`")).collect::<Vec<_>>().join(", ")
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut reasons: Option<Vec<Ident>> = None;
        let mut justification: Option<LitStr> = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "reasons" => {
                    let content;
                    let bracket = syn::bracketed!(content in input);
                    let mut list: Vec<Ident> = Vec::new();
                    while !content.is_empty() {
                        if content.peek(LitStr) {
                            let text: LitStr = content.parse()?;
                            return Err(syn::Error::new(text.span(), format!("name the reason without quotes; the reasons are {}", reason_list())));
                        }
                        let reason: Ident = content.parse()?;
                        if !REASONS.iter().any(|(ident, _)| reason == ident) {
                            let close = REASONS.iter().find(|(ident, chrome)| ident.eq_ignore_ascii_case(&reason.to_string()) || chrome.eq_ignore_ascii_case(&reason.to_string()));
                            let hint = match close {
                                Some((ident, _)) => format!("write `{ident}`"),
                                None => format!("the reasons are {}", reason_list()),
                            };
                            return Err(syn::Error::new(reason.span(), format!("`{reason}` isn't an offscreen reason; {hint}")));
                        }
                        if list.contains(&reason) {
                            return Err(syn::Error::new(reason.span(), format!("`{reason}` is listed twice")));
                        }
                        list.push(reason);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    if list.is_empty() {
                        return Err(syn::Error::new(bracket.span.join(), format!("Chrome needs at least one reason for the offscreen document; the reasons are {}", reason_list())));
                    }
                    reasons = Some(list);
                }
                "justification" => {
                    let text: LitStr = input.parse()?;
                    if text.value().trim().is_empty() {
                        return Err(syn::Error::new(text.span(), "say in a few words why the extension needs the offscreen document; Chrome Web Store reviewers read it"));
                    }
                    justification = Some(text);
                }
                other => return Err(syn::Error::new(key.span(), format!("unknown argument `{other}`; #[oxichrome::offscreen] takes `reasons` and `justification`"))),
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let usage = "write it like #[oxichrome::offscreen(reasons = [DomParser], justification = \"Read the HTML of saved pages\")]";
        Ok(Args {
            reasons: reasons.ok_or_else(|| syn::Error::new(Span::call_site(), format!("missing `reasons`; {usage}")))?,
            justification: justification.ok_or_else(|| syn::Error::new(Span::call_site(), format!("missing `justification`; {usage}")))?,
        })
    }
}

pub fn expand(attr: TokenStream, item: TokenStream, order: crate::codegen::Order) -> Result<TokenStream, syn::Error> {
    let args: Args = syn::parse2(attr)?;
    let mut func: ItemFn = syn::parse2(item)?;
    let what = "#[oxichrome::offscreen] function";
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(func.sig.fn_token, format!("{what} must be async")));
    }
    if !func.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&func.sig.generics, format!("{what} can't be generic")));
    }
    if let Some(receiver) = func.sig.receiver() {
        return Err(syn::Error::new_spanned(receiver, format!("{what} can't take `self`")));
    }

    let fn_name = func.sig.ident.clone();
    let name = fn_name.to_string();
    let vis = func.vis.clone();
    let export = crate::codegen::export_name("offscreen", &fn_name, &order);

    let inputs: Vec<syn::PatType> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => Some(pat_type.clone()),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    if inputs.len() > 1 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "an offscreen function takes at most one argument; put several values in a struct or a tuple",
        ));
    }
    for pat_type in &inputs {
        if borrows(&pat_type.ty) {
            return Err(syn::Error::new_spanned(
                &pat_type.ty,
                "an offscreen function gets a copy of its argument in the offscreen document, so it can't borrow; take an owned type like `String` or `Vec<T>`",
            ));
        }
    }
    let ret_ty = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => {
            if let syn::Type::ImplTrait(_) = ty.as_ref() {
                return Err(syn::Error::new_spanned(ty, "an offscreen function must name its return type; `impl Trait` can't be sent back to the caller"));
            }
            if borrows(ty) {
                return Err(syn::Error::new_spanned(ty, "an offscreen function sends a copy of its result back, so it can't return a borrow; return an owned type like `String`"));
            }
            quote! { #ty }
        }
    };
    let body_name = quote::format_ident!("{}_body", export);
    let (arg_ty, call) = match inputs.first() {
        None => (quote! { () }, quote! { |_: ()| #body_name() }),
        Some(pat_type) => {
            let ty = &pat_type.ty;
            (quote! { #ty }, quote! { #body_name })
        }
    };
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

    let reasons: Vec<&str> = args
        .reasons
        .iter()
        .map(|reason| REASONS.iter().find(|(ident, _)| reason == ident).map(|(_, chrome)| *chrome).expect("checked"))
        .collect();
    let entry = metadata::emit(serde_json::json!({
        "kind": "offscreen",
        "name": name,
        "export": export.to_string(),
        "reasons": reasons,
        "justification": args.justification.value(),
    }));
    let export_str = export.to_string();
    func.sig.ident = body_name.clone();
    func.vis = syn::Visibility::Inherited;
    func.attrs.push(syn::parse_quote!(#[doc(hidden)]));
    let js = quote! { oxichrome::__private::wasm_bindgen::JsValue };

    Ok(quote! {
        #entry
        #arg_check
        #ret_check

        #func

        #[allow(non_snake_case)]
        #vis mod #fn_name {
            #[allow(unused_imports)]
            use super::*;
            pub const OFFSCREEN: oxichrome::offscreen::Offscreen<#arg_ty, #ret_ty> =
                oxichrome::offscreen::Offscreen::__new(#name, #export_str);
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn #export(arg: #js) -> ::core::result::Result<#js, #js> {
            oxichrome::scripting::__run_export::<#arg_ty, #ret_ty, _, _>(#name, arg, #call).await
        }
    })
}
