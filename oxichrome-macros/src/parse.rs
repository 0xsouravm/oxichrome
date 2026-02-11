use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token};

#[derive(Debug)]
pub struct ExtensionArgs {
    pub name: LitStr,
    pub version: LitStr,
    pub description: Option<LitStr>,
    pub permissions: Vec<LitStr>,
}

impl Parse for ExtensionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut version: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut permissions: Vec<LitStr> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    name = Some(input.parse()?);
                }
                "version" => {
                    version = Some(input.parse()?);
                }
                "description" => {
                    description = Some(input.parse()?);
                }
                "permissions" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        permissions.push(content.parse()?);
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown argument `{other}`"),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let name = name.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "missing required argument `name`")
        })?;
        let version = version.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "missing required argument `version`")
        })?;

        Ok(ExtensionArgs {
            name,
            version,
            description,
            permissions,
        })
    }
}

#[derive(Debug)]
pub struct EventArgs {
    pub namespace: Ident,
    pub event_name: Ident,
}

impl Parse for EventArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let namespace: Ident = input.parse()?;
        input.parse::<Token![::]>()?;
        let event_name: Ident = input.parse()?;

        Ok(EventArgs {
            namespace,
            event_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extension_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            name = "Test Extension",
            version = "1.0.0",
            permissions = ["storage", "tabs"]
        };
        let args: ExtensionArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.name.value(), "Test Extension");
        assert_eq!(args.version.value(), "1.0.0");
        assert_eq!(args.permissions.len(), 2);
    }

    #[test]
    fn parse_event_args() {
        let tokens: proc_macro2::TokenStream = quote::quote! {
            runtime::on_installed
        };
        let args: EventArgs = syn::parse2(tokens).unwrap();
        assert_eq!(args.namespace.to_string(), "runtime");
        assert_eq!(args.event_name.to_string(), "on_installed");
    }
}
