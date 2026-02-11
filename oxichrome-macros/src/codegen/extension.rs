use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemStruct;

use crate::parse::ExtensionArgs;

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

    let struct_name = &item_struct.ident;

    Ok(quote! {
        #item_struct

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        mod __oxichrome_meta {
            pub const NAME: &str = #name;
            pub const VERSION: &str = #version;
            pub const DESCRIPTION: Option<&str> = #description;
            pub const PERMISSIONS: &[&str] = &[#(#permissions),*];
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
