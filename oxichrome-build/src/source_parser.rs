//! Parses Rust source files to extract oxichrome metadata without compiling.

use std::path::Path;
use syn::visit::Visit;
use syn::{Attribute, Expr, ExprLit, ItemFn, ItemStruct, Lit, Meta, MetaNameValue};

#[derive(Debug, Default)]
pub struct ExtensionMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub background_functions: Vec<String>,
    pub event_handlers: Vec<EventHandler>,
    pub has_popup: bool,
    pub has_options_page: bool,
}

#[derive(Debug)]
pub struct EventHandler {
    pub fn_name: String,
    pub namespace: String,
    pub event_name: String,
}

struct MetadataVisitor {
    metadata: ExtensionMetadata,
}

impl MetadataVisitor {
    fn new() -> Self {
        MetadataVisitor {
            metadata: ExtensionMetadata::default(),
        }
    }

    fn is_oxichrome_attr(attr: &Attribute, name: &str) -> bool {
        let path = attr.path();
        let segments: Vec<_> = path.segments.iter().collect();
        segments.len() == 2
            && segments[0].ident == "oxichrome"
            && segments[1].ident == name
    }

    fn parse_extension_args(&mut self, attr: &Attribute) {
        let Ok(nested) = attr.parse_args_with(
            syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        ) else {
            return;
        };

        for meta in nested {
            if let Meta::NameValue(MetaNameValue {
                path,
                value: Expr::Lit(ExprLit { lit: Lit::Str(s), .. }),
                ..
            }) = &meta
            {
                if path.is_ident("name") {
                    self.metadata.name = Some(s.value());
                } else if path.is_ident("version") {
                    self.metadata.version = Some(s.value());
                } else if path.is_ident("description") {
                    self.metadata.description = Some(s.value());
                }
            }
            if let Meta::NameValue(MetaNameValue {
                path,
                value,
                ..
            }) = &meta
            {
                if path.is_ident("permissions") {
                    if let Expr::Array(arr) = value {
                        for elem in &arr.elems {
                            if let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = elem {
                                self.metadata.permissions.push(s.value());
                            }
                        }
                    }
                }
            }
        }
    }

    fn parse_event_args(&self, attr: &Attribute) -> Option<(String, String)> {
        let tokens = attr.meta.require_list().ok()?.tokens.clone();
        let parsed: syn::Result<(syn::Ident, syn::Token![::], syn::Ident)> =
            syn::parse2(tokens.clone()).map(|p: EventPath| (p.namespace, p.sep, p.event));

        if let Ok((namespace, _, event)) = parsed {
            return Some((namespace.to_string(), event.to_string()));
        }
        None
    }
}

struct EventPath {
    namespace: syn::Ident,
    #[allow(dead_code)]
    sep: syn::Token![::],
    event: syn::Ident,
}

impl syn::parse::Parse for EventPath {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(EventPath {
            namespace: input.parse()?,
            sep: input.parse()?,
            event: input.parse()?,
        })
    }
}

impl<'ast> Visit<'ast> for MetadataVisitor {
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        for attr in &node.attrs {
            if Self::is_oxichrome_attr(attr, "extension") {
                self.parse_extension_args(attr);
            }
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        for attr in &node.attrs {
            if Self::is_oxichrome_attr(attr, "background") {
                self.metadata
                    .background_functions
                    .push(node.sig.ident.to_string());
            }
            if Self::is_oxichrome_attr(attr, "on") {
                if let Some((namespace, event_name)) = self.parse_event_args(attr) {
                    self.metadata.event_handlers.push(EventHandler {
                        fn_name: node.sig.ident.to_string(),
                        namespace,
                        event_name,
                    });
                }
            }
            if Self::is_oxichrome_attr(attr, "popup") {
                self.metadata.has_popup = true;
            }
            if Self::is_oxichrome_attr(attr, "options_page") {
                self.metadata.has_options_page = true;
            }
        }
        syn::visit::visit_item_fn(self, node);
    }
}

pub fn parse_source(path: &Path) -> anyhow::Result<ExtensionMetadata> {
    let source = std::fs::read_to_string(path)?;
    parse_source_str(&source)
}

pub fn parse_source_str(source: &str) -> anyhow::Result<ExtensionMetadata> {
    let file = syn::parse_file(source)?;
    let mut visitor = MetadataVisitor::new();
    visitor.visit_file(&file);
    Ok(visitor.metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extension() {
        let source = r#"
            use oxichrome::prelude::*;

            #[oxichrome::extension(
                name = "Test Extension",
                version = "1.0.0",
                permissions = ["storage", "tabs"]
            )]
            struct MyExt;

            #[oxichrome::background]
            async fn start() {}

            #[oxichrome::on(runtime::on_installed)]
            async fn handle_install(details: JsValue) {}

            #[oxichrome::popup]
            fn Popup() -> impl IntoView {}

            #[oxichrome::options_page]
            fn Options() -> impl IntoView {}
        "#;

        let metadata = parse_source_str(source).unwrap();
        assert_eq!(metadata.name.as_deref(), Some("Test Extension"));
        assert_eq!(metadata.version.as_deref(), Some("1.0.0"));
        assert_eq!(metadata.permissions, vec!["storage", "tabs"]);
        assert_eq!(metadata.background_functions, vec!["start"]);
        assert_eq!(metadata.event_handlers.len(), 1);
        assert_eq!(metadata.event_handlers[0].namespace, "runtime");
        assert_eq!(metadata.event_handlers[0].event_name, "on_installed");
        assert!(metadata.has_popup);
        assert!(metadata.has_options_page);
    }
}
