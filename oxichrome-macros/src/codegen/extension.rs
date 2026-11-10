use proc_macro2::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{ItemStruct, LitStr};

use crate::metadata;
use crate::parse::{normalize_icon_path, png_dimensions, ExtensionArgs, IconSpec};

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

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let args: ExtensionArgs = syn::parse2(attr)?;
    let item_struct: ItemStruct = syn::parse2(item)?;

    let name = &args.name;
    let version = &args.version;

    let description = match &args.description {
        Some(desc) => quote! { Some(#desc) },
        None => quote! { None::<&str> },
    };

    let permissions: Vec<&str> = args.permissions.iter().map(|declared| declared.info.name).collect();
    let host_permissions: Vec<_> = args.host_permissions.iter().collect();
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
            "default_title": action.default_title.as_ref().map(LitStr::value),
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
        "name": name.value(),
        "version": version.value(),
        "description": args.description.as_ref().map(LitStr::value),
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

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        mod __oxichrome_meta {
            pub const NAME: &str = #name;
            pub const VERSION: &str = #version;
            pub const DESCRIPTION: Option<&str> = #description;
            pub const PERMISSIONS: &[&str] = &[#(#permissions),*];
            pub const HOST_PERMISSIONS: &[&str] = &[#(#host_permissions),*];
        }

        #[doc(hidden)]
        #[allow(dead_code)]
        const __OXICHROME_EXTENSION_MARKER: &str = concat!(
            "__OXICHROME__",
            #name, "__",
            #version, "__",
        );

        impl #struct_name {
            #[allow(dead_code)]
            pub fn name() -> &'static str {
                #name
            }

            #[allow(dead_code)]
            pub fn version() -> &'static str {
                #version
            }
        }
    })
}
