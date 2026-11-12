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

const GENERATED_PAGES: &[&str] = &["popup.html", "options.html", "sidepanel.html"];

fn static_root(span: proc_macro2::Span) -> syn::Result<PathBuf> {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(|dir| PathBuf::from(dir).join("static"))
        .ok_or_else(|| syn::Error::new(span, "CARGO_MANIFEST_DIR is not set"))
}

fn track(relative: &str) -> TokenStream {
    let include = format!("/static/{relative}");
    quote! {
        #[doc(hidden)]
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), #include));
    }
}

fn check_resource(relative: &str, span: proc_macro2::Span) -> syn::Result<TokenStream> {
    if GENERATED_PAGES.contains(&relative) {
        return Ok(TokenStream::new());
    }
    let root = static_root(span)?;
    if relative.contains('*') {
        if let Some((folder, _)) = relative.rsplit_once('/') {
            let found = find_exact(&root, folder).map_err(|reason| syn::Error::new(span, reason))?;
            if !found.is_dir() {
                return Err(syn::Error::new(span, format!("`static/{folder}` isn't a folder")));
            }
        }
        return Ok(TokenStream::new());
    }
    let file = find_exact(&root, relative).map_err(|reason| syn::Error::new(span, reason))?;
    if !file.is_file() {
        return Err(syn::Error::new(span, format!("`static/{relative}` is a folder; name a file, or `{relative}/*` for everything in it")));
    }
    Ok(track(relative))
}

struct RuleSets {
    checks: TokenStream,
    entries: Vec<serde_json::Value>,
    extension_paths: Vec<String>,
}

fn check_rule_sets(args: &ExtensionArgs) -> syn::Result<RuleSets> {
    use oxichrome_core::declarative_net_request as dnr;
    let mut checks = TokenStream::new();
    let mut entries = Vec::new();
    let mut extension_paths: Vec<String> = Vec::new();
    let mut enabled_rules = 0;
    let mut needs_host_access: Option<&LitStr> = None;
    for (id, path) in &args.rule_sets {
        let relative = path.value();
        let root = static_root(path.span())?;
        let file = find_exact(&root, &relative).map_err(|reason| syn::Error::new(path.span(), format!("rule set `{id}`: {reason}")))?;
        let text = std::fs::read_to_string(&file).map_err(|err| syn::Error::new(path.span(), format!("rule set `{id}`: `static/{relative}` can't be read: {err}")))?;
        let summary = dnr::check_static_rule_set(&text).map_err(|why| syn::Error::new(path.span(), format!("rule set `{id}` in `static/{relative}`: {why}")))?;
        let enabled = !args.disabled_rule_sets.contains(id);
        if enabled {
            enabled_rules += summary.rules;
        }
        if summary.needs_host_access && needs_host_access.is_none() {
            needs_host_access = Some(path);
        }
        for target in &summary.extension_paths {
            let inside = target.trim_start_matches('/').to_string();
            check_resource(&inside, path.span()).map_err(|err| syn::Error::new(path.span(), format!("rule set `{id}` redirects to `{target}`, but {err}")))?;
            if !extension_paths.contains(&inside) {
                extension_paths.push(inside);
            }
        }
        checks.extend(track(&relative));
        entries.push(serde_json::json!({ "id": id.to_string(), "path": relative, "enabled": enabled }));
    }
    if enabled_rules > dnr::GUARANTEED_STATIC_RULES {
        let span = args.rule_sets[0].1.span();
        return Err(syn::Error::new(span, format!(
            "the rule sets on at install have {enabled_rules} rules; both browsers only promise {} for one extension, so turn some off with `disabled_rule_sets`",
            dnr::GUARANTEED_STATIC_RULES
        )));
    }
    let has_hosts = !args.host_permissions.is_empty() || !args.optional_host_permissions.is_empty();
    if let Some(path) = needs_host_access {
        if !has_hosts {
            return Err(syn::Error::new(path.span(), format!(
                "`static/{}` has redirect or modifyHeaders rules, which only work on sites the extension has host permission for; add them to `host_permissions`",
                path.value()
            )));
        }
    }
    let only_with_host_access = args.permissions.iter().any(|declared| declared.info.ident == "DeclarativeNetRequestWithHostAccess")
        && !args.permissions.iter().any(|declared| declared.info.ident == "DeclarativeNetRequest");
    if only_with_host_access && !has_hosts && !args.rule_sets.is_empty() {
        return Err(syn::Error::new(args.rule_sets[0].1.span(), "`DeclarativeNetRequestWithHostAccess` only acts on sites in `host_permissions`, and there are none; add sites there, or use `DeclarativeNetRequest`"));
    }
    Ok(RuleSets { checks, entries, extension_paths })
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
    let rule_sets = check_rule_sets(&args)?;
    let mut web_accessible: Vec<String> = Vec::new();
    let mut resource_checks = rule_sets.checks;
    for path in &args.web_accessible {
        resource_checks.extend(check_resource(&path.value(), path.span())?);
        web_accessible.push(path.value());
    }
    for path in rule_sets.extension_paths {
        if !web_accessible.contains(&path) {
            web_accessible.push(path);
        }
    }

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
        "rule_sets": rule_sets.entries,
        "web_accessible": web_accessible,
    }));

    Ok(quote! {
        #item_struct

        #entry

        #(#icon_checks)*
        #resource_checks
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
