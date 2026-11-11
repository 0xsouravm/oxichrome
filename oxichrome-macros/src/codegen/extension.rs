use proc_macro2::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{ItemStruct, LitStr};

use crate::metadata;
use crate::parse::{normalize_icon_path, png_dimensions, ExtensionArgs, IconSpec, Text};

fn find_exact(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();
    for segment in relative.split('/') {
        let entries = std::fs::read_dir(&current)
            .map_err(|_| format!("`static/{relative}` doesn't exist"))?;
        let names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        if !names.iter().any(|name| name == segment) {
            if let Some(other) = names.iter().find(|name| name.eq_ignore_ascii_case(segment)) {
                return Err(format!(
                    "`{segment}` is named `{other}` on disk; the case must match exactly, because packaged extensions are case-sensitive"
                ));
            }
            return Err(format!("`static/{relative}` doesn't exist"));
        }
        current.push(segment);
    }
    Ok(current)
}

fn check_icon(path: &LitStr) -> syn::Result<TokenStream> {
    let relative = normalize_icon_path(path)?;
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| syn::Error::new(path.span(), "CARGO_MANIFEST_DIR is not set"))?;
    let file = find_exact(&root.join("static"), &relative)
        .map_err(|reason| syn::Error::new(path.span(), format!("icon not found: {reason}")))?;
    if !file.is_file() {
        return Err(syn::Error::new(
            path.span(),
            format!("icon `static/{relative}` is not a file"),
        ));
    }
    let bytes = std::fs::read(&file).map_err(|err| {
        syn::Error::new(
            path.span(),
            format!("icon `static/{relative}` can't be read: {err}"),
        )
    })?;
    if png_dimensions(&bytes).is_none() {
        return Err(syn::Error::new(
            path.span(),
            format!("icon `static/{relative}` is not a valid PNG image"),
        ));
    }
    let include = format!("/static/{relative}");
    Ok(quote! {
        #[doc(hidden)]
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), #include));
    })
}

fn check_locales(args: &ExtensionArgs) -> syn::Result<(TokenStream, String)> {
    let locales = crate::i18n::load().map_err(|err| {
        syn::Error::new(args.default_locale.as_ref().map(LitStr::span).unwrap_or_else(proc_macro2::Span::call_site), err)
    })?;
    let mut texts: Vec<&Text> = vec![&args.name];
    texts.extend(args.description.as_ref());
    texts.extend(args.action.as_ref().and_then(|action| action.default_title.as_ref()));
    let display_name = match &args.default_locale {
        None => {
            if let Some(Text::Message { span, .. }) = texts.iter().find(|text| matches!(text, Text::Message { .. })) {
                return Err(syn::Error::new(*span, "add `default_locale = \"en\"` (or the language your messages are in) to use messages here"));
            }
            if locales.exists {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "`static/_locales` is there, so add `default_locale = \"en\"` (or the language your messages are in); Chrome refuses to load the extension without it",
                ));
            }
            args.name.value()
        }
        Some(default) => {
            locales.check_default(&default.value()).map_err(|err| syn::Error::new(default.span(), err))?;
            let base = locales.get(&default.value()).expect("checked");
            for text in &texts {
                if let Text::Message { key, span, .. } = text {
                    if !base.messages.iter().any(|message| &message.key == key) {
                        return Err(syn::Error::new(*span, format!("`{key}` isn't in the default locale `{}`; add it to `static/_locales/{}/messages.json`", default.value(), default.value())));
                    }
                }
            }
            match &args.name {
                Text::Literal(text) => text.value(),
                Text::Message { key, .. } => base.messages.iter().find(|message| &message.key == key).expect("checked").text.clone(),
            }
        }
    };
    let mut checks = crate::i18n::tracking(&locales);
    for text in texts {
        checks.extend(text.checks("#[oxichrome::extension]"));
    }
    Ok((checks, display_name))
}

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: ExtensionArgs = syn::parse2(attr)?;
    let item_struct: ItemStruct = syn::parse2(item)?;

    let version = &args.version;
    let (locale_checks, display_name) = check_locales(&args)?;

    let permission_checks = args.permissions.iter().chain(&args.optional_permissions).map(|declared| {
        let ident = &declared.ident;
        quote! {
            #[doc(hidden)]
            const _: oxichrome::Permission = oxichrome::Permission::#ident;
        }
    });
    let names = |list: &[crate::permission::Declared]| list.iter().map(|declared| declared.info.name).collect::<Vec<_>>();
    let only = |browser: fn(&crate::permission::Info) -> crate::permission::Support| {
        args.permissions
            .iter()
            .chain(&args.optional_permissions)
            .filter(|declared| browser(declared.info) == crate::permission::Support::No)
            .map(|declared| [declared.info.name, declared.info.ident])
            .collect::<Vec<_>>()
    };

    let struct_name = &item_struct.ident;

    let mut icon_paths: Vec<&LitStr> = args.icons.iter().map(|(_, path)| path).collect();
    if let Some(icon) = args
        .action
        .as_ref()
        .and_then(|action| action.default_icon.as_ref())
    {
        icon_paths.extend(icon.paths());
    }
    let icon_checks = icon_paths
        .into_iter()
        .map(check_icon)
        .collect::<syn::Result<Vec<_>>>()?;

    let sizes = |sizes: &[(u32, LitStr)]| -> syn::Result<serde_json::Value> {
        sizes
            .iter()
            .map(|(size, path)| Ok(serde_json::json!([size, normalize_icon_path(path)?])))
            .collect::<syn::Result<Vec<_>>>()
            .map(serde_json::Value::Array)
    };
    let action = match &args.action {
        None => serde_json::Value::Null,
        Some(action) => serde_json::json!({
            "default_title": action.default_title.as_ref().map(Text::value),
            "default_icon": match &action.default_icon {
                None => serde_json::Value::Null,
                Some(IconSpec::Single(path)) => serde_json::json!(normalize_icon_path(path)?),
                Some(IconSpec::Sizes(icons)) => sizes(icons)?,
            },
            "shortcut": action.shortcut.as_ref().map(|shortcut| shortcut.to_json()),
        }),
    };
    let entry = metadata::emit(serde_json::json!({
        "kind": "extension",
        "name": args.name.value(),
        "display_name": display_name,
        "default_locale": args.default_locale.as_ref().map(LitStr::value),
        "version": version.value(),
        "description": args.description.as_ref().map(Text::value),
        "gecko_id": args.gecko_id.as_ref().map(LitStr::value),
        "permissions": names(&args.permissions),
        "optional_permissions": names(&args.optional_permissions),
        "host_permissions": args.host_permissions.iter().map(LitStr::value).collect::<Vec<_>>(),
        "optional_host_permissions": args.optional_host_permissions.iter().map(LitStr::value).collect::<Vec<_>>(),
        "chrome_only": only(|info| info.firefox),
        "firefox_only": only(|info| info.chrome),
        "icons": sizes(&args.icons)?,
        "action": action,
    }));

    Ok(quote! {
        #item_struct

        #entry

        #(#icon_checks)*
        #(#permission_checks)*
        #locale_checks

        impl #struct_name {
            #[allow(dead_code)]
            pub fn name() -> &'static str {
                #display_name
            }

            #[allow(dead_code)]
            pub fn version() -> &'static str {
                #version
            }
        }
    })
}
