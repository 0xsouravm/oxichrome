use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{FnArg, Ident, ItemFn, ItemStruct, LitStr};

use crate::codegen::{export_name, Order};
use crate::metadata;
use crate::parse::{parse_menu_args, MenuArgs, MenuKind};

fn context_mask(args: &MenuArgs) -> Option<TokenStream> {
    let contexts = args.contexts.as_ref()?;
    let flags = contexts.iter().map(|context| {
        let flag = format_ident!("{}", context.to_string().to_uppercase());
        quote! { oxichrome::context_menus::contexts::#flag }
    });
    Some(quote! { 0 #(| #flags)* })
}

fn selection_only(args: &MenuArgs) -> bool {
    args.contexts
        .as_ref()
        .is_some_and(|contexts| contexts.len() == 1 && contexts[0] == "selection")
}

fn entry(args: &MenuArgs, id: &Ident, item_type: &str, handler: Option<&Ident>, export: Option<&Ident>, order: &Order) -> TokenStream {
    let checked = match args.kind {
        MenuKind::Normal => None,
        MenuKind::Checkbox(checked) | MenuKind::Radio(checked) => Some(checked),
    };
    let parent = args
        .parent
        .as_ref()
        .and_then(|path| path.segments.last())
        .map(|segment| segment.ident.to_string());
    let at = format!("{}:{}:{}", order.file, order.line, order.column);
    let checks = args.title.checks(&at);
    let entry = metadata::emit(serde_json::json!({
        "kind": "menu",
        "id": id.to_string(),
        "handler": handler.map(Ident::to_string),
        "export": export.map(Ident::to_string),
        "item_type": item_type,
        "title": args.title.value(),
        "contexts": args.contexts.as_ref().map(|contexts| contexts.iter().map(Ident::to_string).collect::<Vec<_>>()),
        "checked": checked,
        "parent": parent,
        "enabled": args.enabled,
        "visible": args.visible,
        "document_url_patterns": args.document_url_patterns.iter().map(LitStr::value).collect::<Vec<_>>(),
        "target_url_patterns": args.target_url_patterns.iter().map(LitStr::value).collect::<Vec<_>>(),
        "file": order.file,
        "line": order.line,
        "column": order.column,
    }));
    quote! { #entry #checks }
}

fn parent_check(args: &MenuArgs) -> TokenStream {
    let Some(parent) = &args.parent else {
        return TokenStream::new();
    };
    let parent_contexts = quote_spanned! {parent.span()=>
        <#parent as oxichrome::context_menus::IsSubmenu>::CONTEXTS
    };
    match context_mask(args) {
        Some(mask) => quote! {
            #[doc(hidden)]
            const _: () = assert!(
                (#mask) & !#parent_contexts == 0,
                "this menu item lists a context its parent submenu doesn't; an item only shows where its submenu shows, so remove that context or add it to the submenu"
            );
        },
        None => quote! {
            #[doc(hidden)]
            const _: u16 = #parent_contexts;
        },
    }
}

fn parse_args(attr: TokenStream, submenu: bool) -> syn::Result<MenuArgs> {
    syn::parse::Parser::parse2(
        |input: syn::parse::ParseStream| parse_menu_args(input, submenu),
        attr,
    )
}

pub fn expand_menu(attr: TokenStream, item: TokenStream, order: Order) -> syn::Result<TokenStream> {
    let args = parse_args(attr, false)?;
    let func: ItemFn = syn::parse2(item)?;

    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "#[oxichrome::menu(...)] function must be async",
        ));
    }
    if !func.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig.generics,
            "#[oxichrome::menu(...)] function can't be generic",
        ));
    }
    let mut arg_types = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pat_type) => arg_types.push(pat_type.ty.as_ref().clone()),
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "#[oxichrome::menu(...)] function can't take `self`",
                ))
            }
        }
    }
    if arg_types.len() > 1 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "a menu handler takes no arguments or one `context_menus::Click`",
        ));
    }

    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let id = fn_name.to_string();
    let selection_only = selection_only(&args);
    let kind = match args.kind {
        MenuKind::Normal => quote! { Normal },
        MenuKind::Checkbox(_) => quote! { Checkbox },
        MenuKind::Radio(_) => quote! { Radio },
    };
    let item_type = match args.kind {
        MenuKind::Normal => "normal",
        MenuKind::Checkbox(_) => "checkbox",
        MenuKind::Radio(_) => "radio",
    };
    let export = export_name("menu", fn_name, &order);
    let entry = entry(&args, fn_name, item_type, Some(fn_name), Some(&export), &order);
    let parent_check = parent_check(&args);

    let js_type = quote! { oxichrome::__private::wasm_bindgen::JsValue };
    let call = match arg_types.first() {
        None => quote! { #fn_name().await; },
        Some(ty) => {
            let handler = fn_name.to_string();
            let check = quote_spanned! {ty.span()=>
                oxichrome::core::event::assert_arg::<#ty, oxichrome::core::event::events::ContextMenusOnClicked, 1>();
            };
            quote! {
                #check
                let __oxichrome_click = <#ty as oxichrome::core::event::FromEventArg>::from_event_arg(__oxichrome_click)
                    .map_err(|err| {
                        #js_type::from(oxichrome::__private::js_sys::Error::new(&format!(
                            "[oxichrome] {} argument 1 could not be read: {}",
                            #handler, err
                        )))
                    })?;
                #fn_name(__oxichrome_click).await;
            }
        }
    };

    Ok(quote! {
        #func

        #entry

        #parent_check

        #[doc(hidden)]
        #vis mod #fn_name {
            pub const ITEM: oxichrome::context_menus::MenuItem<oxichrome::context_menus::#kind> =
                oxichrome::context_menus::MenuItem::__new(#id, #selection_only);
        }

        #[doc(hidden)]
        #[oxichrome::__private::wasm_bindgen::prelude::wasm_bindgen(wasm_bindgen = oxichrome::__private::wasm_bindgen, wasm_bindgen_futures = oxichrome::__private::wasm_bindgen_futures)]
        pub fn #export(__oxichrome_click: #js_type) -> oxichrome::__private::js_sys::Promise {
            oxichrome::__private::wasm_bindgen_futures::future_to_promise(async move {
                let _ = &__oxichrome_click;
                #call
                Ok(#js_type::UNDEFINED)
            })
        }
    })
}

pub fn expand_submenu(attr: TokenStream, item: TokenStream, order: Order) -> syn::Result<TokenStream> {
    let args = parse_args(attr, true)?;
    let item_struct: ItemStruct = syn::parse2(item)?;
    if !matches!(item_struct.fields, syn::Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &item_struct,
            "#[oxichrome::submenu(...)] goes on a unit struct, like `struct Tools;`",
        ));
    }
    if !item_struct.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item_struct.generics,
            "#[oxichrome::submenu(...)] struct can't be generic",
        ));
    }
    let name = &item_struct.ident;
    let id = name.to_string();
    let selection_only = selection_only(&args);
    let entry = entry(&args, name, "submenu", None, None, &order);
    let parent_check = parent_check(&args);
    let contexts = match (context_mask(&args), &args.parent) {
        (Some(mask), _) => mask,
        (None, Some(parent)) => quote! { <#parent as oxichrome::context_menus::IsSubmenu>::CONTEXTS },
        (None, None) => quote! { oxichrome::context_menus::contexts::PAGE },
    };

    Ok(quote! {
        #item_struct

        #entry

        #parent_check

        impl oxichrome::context_menus::IsSubmenu for #name {
            const CONTEXTS: u16 = #contexts;
        }

        impl #name {
            #[allow(dead_code)]
            pub const ITEM: oxichrome::context_menus::MenuItem<oxichrome::context_menus::Submenu> =
                oxichrome::context_menus::MenuItem::__new(#id, #selection_only);
        }
    })
}
