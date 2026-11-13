use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::parse::{Parse, ParseStream};
use syn::{FnArg, ItemFn, LitStr, ReturnType};

use crate::metadata;

struct Args {
    keyword: LitStr,
    default: Option<LitStr>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut keyword: Option<LitStr> = None;
        let mut default: Option<LitStr> = None;
        let start = input.span();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>().map_err(|err| syn::Error::new(err.span(), format!("write `{key} = \"...\"`")))?;
            let target = match key.to_string().as_str() {
                "keyword" => &mut keyword,
                "default" => &mut default,
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown omnibox option `{other}`; use `keyword = \"...\"` and optionally `default = \"... {{input}} ...\"`"),
                    ))
                }
            };
            if target.is_some() {
                return Err(syn::Error::new(key.span(), format!("`{key}` is set twice")));
            }
            let value: LitStr = if input.peek(LitStr) {
                input.parse()?
            } else {
                return Err(syn::Error::new(input.span(), format!("`{key}` takes a string like \"...\"")));
            };
            *target = Some(value);
            if !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
            }
        }
        let keyword = keyword.ok_or_else(|| {
            syn::Error::new(start, "add `keyword = \"...\"`: the word people type in the address bar before their search")
        })?;
        let word = keyword.value();
        if word.is_empty() {
            return Err(syn::Error::new(keyword.span(), "the keyword is empty"));
        }
        if let Some(bad) = word.chars().find(|c| c.is_whitespace() || c.is_control()) {
            return Err(syn::Error::new(
                keyword.span(),
                format!("the keyword can't contain {}; it's one word people type before a space", if bad == ' ' { "spaces".to_string() } else { format!("{bad:?}") }),
            ));
        }
        if let Some(default) = &default {
            let text = default.value();
            if text.trim().is_empty() {
                return Err(syn::Error::new(default.span(), "the default suggestion is empty; leave `default` out instead"));
            }
            if text.contains("%s") {
                return Err(syn::Error::new(default.span(), "write `{input}` where the typed text goes; `%s` only works in Chrome"));
            }
            if text.replace("{input}", "").trim().is_empty() {
                return Err(syn::Error::new(default.span(), "add words around `{input}`, like \"Search docs for {input}\"; Firefox can't show the typed text there, so it shows only the words"));
            }
            if text.matches("{input}").count() > 1 {
                return Err(syn::Error::new(default.span(), "use `{input}` once; Chrome fills in only the first"));
            }
            if text.chars().any(|c| c.is_control()) {
                return Err(syn::Error::new(default.span(), "the default suggestion is one line; remove line breaks and tabs"));
            }
        }
        Ok(Args { keyword, default })
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: Args = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;
    let shape = "write it as `async fn suggest(input: String) -> Vec<omnibox::Suggestion>`";
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(&func.sig.fn_token, format!("the omnibox function must be async; {shape}")));
    }
    let input_ty = match func.sig.inputs.iter().collect::<Vec<_>>().as_slice() {
        [FnArg::Typed(input)] => input.ty.clone(),
        _ => return Err(syn::Error::new_spanned(&func.sig.inputs, format!("the omnibox function takes the typed text; {shape}"))),
    };
    let ReturnType::Type(_, output_ty) = &func.sig.output else {
        return Err(syn::Error::new_spanned(&func.sig, format!("return the suggestions to show; {shape}")));
    };
    let fn_name = &func.sig.ident;
    let name = fn_name.to_string();
    let from_text = quote_spanned! {input_ty.span()=> <#input_ty as oxichrome::omnibox::__Input>::__from_text(__oxichrome_input)};
    let into_list = quote_spanned! {output_ty.span()=> <#output_ty as oxichrome::omnibox::__Output>::__into_list(__oxichrome_result)};
    let entry = metadata::emit(serde_json::json!({
        "kind": "omnibox",
        "fn_name": name,
        "keyword": args.keyword.value(),
        "default": args.default.as_ref().map(LitStr::value),
    }));

    Ok(quote! {
        #entry

        #func

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(js_name = __oxichrome_omnibox_suggest, wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn __oxichrome_only_one_omnibox_per_extension(__oxichrome_input: String) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                let __oxichrome_result = #fn_name(#from_text).await;
                Ok(oxichrome::omnibox::__prepare(#name, #into_list))
            })
        }
    })
}
