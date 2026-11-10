use proc_macro2::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{ItemStruct, LitStr};

use crate::parse::{normalize_icon_path, png_dimensions, ExtensionArgs};

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

    let permissions: Vec<_> = args.permissions.iter().collect();
    let host_permissions: Vec<_> = args.host_permissions.iter().collect();

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

    Ok(quote! {
        #item_struct

        #(#icon_checks)*

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
