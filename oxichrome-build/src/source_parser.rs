//! Parses Rust source files to extract oxichrome metadata without compiling.

use std::path::Path;
use syn::parse::{Parse, ParseStream};
use syn::visit::Visit;
use syn::{Attribute, Expr, ExprLit, ItemFn, ItemStruct, Lit, Meta, MetaNameValue};

#[derive(Debug, Default)]
pub struct ExtensionMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub host_permissions: Vec<String>,
    pub background_functions: Vec<String>,
    pub event_handlers: Vec<EventHandler>,
    pub has_popup: bool,
    pub has_options_page: bool,
    pub content_scripts: Vec<ContentScript>,
    pub gecko_id: Option<String>,
    pub icons: Vec<(u32, String)>,
    pub action: Option<ActionMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActionMetadata {
    pub default_title: Option<String>,
    pub default_icon: Option<ActionIcon>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionIcon {
    Single(String),
    Sizes(Vec<(u32, String)>),
}

enum ArgValue {
    Str(String),
    List(Vec<String>),
    Map(Vec<(String, ArgValue)>),
    Other,
}

impl Parse for ArgValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            return Ok(ArgValue::Str(input.parse::<syn::LitStr>()?.value()));
        }
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let items =
                syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated(
                    &content,
                )?;
            return Ok(ArgValue::List(
                items.iter().map(|item| item.value()).collect(),
            ));
        }
        if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            let mut entries = Vec::new();
            while !content.is_empty() {
                let key = if content.peek(syn::LitInt) {
                    content.parse::<syn::LitInt>()?.base10_digits().to_string()
                } else if content.peek(syn::LitStr) {
                    content.parse::<syn::LitStr>()?.value()
                } else {
                    content.parse::<syn::Ident>()?.to_string()
                };
                content.parse::<syn::Token![=]>()?;
                entries.push((key, content.parse()?));
                if !content.is_empty() {
                    content.parse::<syn::Token![,]>()?;
                }
            }
            return Ok(ArgValue::Map(entries));
        }
        input.parse::<syn::Expr>()?;
        Ok(ArgValue::Other)
    }
}

struct ExtensionArg {
    key: String,
    value: ArgValue,
}

impl Parse for ExtensionArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key = input.parse::<syn::Ident>()?.to_string();
        input.parse::<syn::Token![=]>()?;
        Ok(ExtensionArg {
            key,
            value: input.parse()?,
        })
    }
}

fn icon_path(path: &str) -> String {
    let mut rest = path;
    loop {
        if let Some(stripped) = rest.strip_prefix("./") {
            rest = stripped;
        } else if let Some(stripped) = rest.strip_prefix('/') {
            rest = stripped;
        } else {
            return rest.to_string();
        }
    }
}

fn icon_sizes(entries: &[(String, ArgValue)]) -> Vec<(u32, String)> {
    let mut sizes: Vec<(u32, String)> = entries
        .iter()
        .filter_map(|(size, path)| match path {
            ArgValue::Str(path) => Some((size.parse().ok()?, icon_path(path))),
            _ => None,
        })
        .collect();
    sizes.sort_by_key(|(size, _)| *size);
    sizes
}

#[derive(Debug)]
pub struct EventHandler {
    pub fn_name: String,
    pub namespace: String,
    pub event_name: String,
}

#[derive(Debug)]
pub struct ContentScript {
    pub fn_name: String,
    pub matches: Vec<String>,
    pub run_at: Option<String>,
    pub all_frames: Option<bool>,
    pub css: Vec<String>,
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
        segments.len() == 2 && segments[0].ident == "oxichrome" && segments[1].ident == name
    }

    fn parse_extension_args(&mut self, attr: &Attribute) {
        let Ok(args) = attr.parse_args_with(
            syn::punctuated::Punctuated::<ExtensionArg, syn::Token![,]>::parse_terminated,
        ) else {
            return;
        };

        for arg in args {
            match (arg.key.as_str(), arg.value) {
                ("name", ArgValue::Str(value)) => self.metadata.name = Some(value),
                ("version", ArgValue::Str(value)) => self.metadata.version = Some(value),
                ("description", ArgValue::Str(value)) => self.metadata.description = Some(value),
                ("gecko_id", ArgValue::Str(value)) => self.metadata.gecko_id = Some(value),
                ("permissions", ArgValue::List(values)) => self.metadata.permissions = values,
                ("host_permissions", ArgValue::List(values)) => {
                    self.metadata.host_permissions = values
                }
                ("icons", ArgValue::Map(entries)) => self.metadata.icons = icon_sizes(&entries),
                ("action", ArgValue::Map(entries)) => {
                    let mut action = ActionMetadata::default();
                    for (key, value) in entries {
                        match (key.as_str(), value) {
                            ("default_title", ArgValue::Str(title)) => {
                                action.default_title = Some(title)
                            }
                            ("default_icon", ArgValue::Str(path)) => {
                                action.default_icon = Some(ActionIcon::Single(icon_path(&path)))
                            }
                            ("default_icon", ArgValue::Map(sizes)) => {
                                action.default_icon = Some(ActionIcon::Sizes(icon_sizes(&sizes)))
                            }
                            _ => {}
                        }
                    }
                    self.metadata.action = Some(action);
                }
                _ => {}
            }
        }
    }

    fn parse_content_script_args(&self, attr: &Attribute) -> Option<ContentScript> {
        let nested = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            .ok()?;

        let mut matches = Vec::new();
        let mut run_at = None;
        let mut all_frames = None;
        let mut css = Vec::new();

        for meta in &nested {
            if let Meta::NameValue(MetaNameValue { path, value, .. }) = meta {
                if path.is_ident("matches") {
                    if let Expr::Array(arr) = value {
                        for elem in &arr.elems {
                            if let Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            }) = elem
                            {
                                matches.push(s.value());
                            }
                        }
                    }
                } else if path.is_ident("run_at") {
                    if let Expr::Path(expr_path) = value {
                        if let Some(ident) = expr_path.path.get_ident() {
                            run_at = match ident.to_string().as_str() {
                                "DocumentStart" => Some("document_start".to_string()),
                                "DocumentEnd" => Some("document_end".to_string()),
                                "DocumentIdle" => Some("document_idle".to_string()),
                                _ => None,
                            };
                        }
                    }
                } else if path.is_ident("all_frames") {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Bool(b), ..
                    }) = value
                    {
                        all_frames = Some(b.value());
                    }
                } else if path.is_ident("css") {
                    if let Expr::Array(arr) = value {
                        for elem in &arr.elems {
                            if let Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            }) = elem
                            {
                                css.push(s.value());
                            }
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            return None;
        }

        Some(ContentScript {
            fn_name: String::new(), // filled in by caller
            matches,
            run_at,
            all_frames,
            css,
        })
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
            if Self::is_oxichrome_attr(attr, "content_script") {
                if let Some(mut cs) = self.parse_content_script_args(attr) {
                    cs.fn_name = node.sig.ident.to_string();
                    self.metadata.content_scripts.push(cs);
                }
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
    #[test]
    fn test_parse_icons_and_action_with_other_args() {
        let source = r#"
            #[oxichrome::extension(
                name = "Icons",
                version = "1.2.3",
                permissions = ["storage"],
                icons = { 128 = "icons/128.png", 16 = "./icons/16.png" },
                action = { default_title = "Click", default_icon = { 32 = "/icons/32.png" } },
                host_permissions = ["https://example.org/*"],
                gecko_id = "icons@example.com",
            )]
            struct Ext;
        "#;
        let metadata = parse_source_str(source).unwrap();
        assert_eq!(metadata.name.as_deref(), Some("Icons"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.permissions, vec!["storage"]);
        assert_eq!(metadata.host_permissions, vec!["https://example.org/*"]);
        assert_eq!(metadata.gecko_id.as_deref(), Some("icons@example.com"));
        assert_eq!(
            metadata.icons,
            vec![
                (16, "icons/16.png".to_string()),
                (128, "icons/128.png".to_string())
            ]
        );
        assert_eq!(
            metadata.action,
            Some(ActionMetadata {
                default_title: Some("Click".to_string()),
                default_icon: Some(ActionIcon::Sizes(vec![(32, "icons/32.png".to_string())])),
            })
        );
    }

    #[test]
    fn test_parse_single_action_icon() {
        let source = r#"
            #[oxichrome::extension(name = "A", version = "1.0.0", action = { default_icon = "a.png" })]
            struct Ext;
        "#;
        let metadata = parse_source_str(source).unwrap();
        assert_eq!(
            metadata.action.unwrap().default_icon,
            Some(ActionIcon::Single("a.png".to_string()))
        );
        assert!(metadata.icons.is_empty());
    }
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
        assert!(metadata.content_scripts.is_empty());
    }

    #[test]
    fn test_parse_content_script() {
        let source = r#"
            #[oxichrome::extension(
                name = "Test",
                version = "1.0.0"
            )]
            struct MyExt;

            #[oxichrome::content_script(matches = ["<all_urls>"])]
            async fn inject() {}

            #[oxichrome::content_script(matches = ["https://example.com/*", "https://test.com/*"])]
            async fn inject_specific() {}
        "#;

        let metadata = parse_source_str(source).unwrap();
        assert_eq!(metadata.content_scripts.len(), 2);
        assert_eq!(metadata.content_scripts[0].fn_name, "inject");
        assert_eq!(metadata.content_scripts[0].matches, vec!["<all_urls>"]);
        assert_eq!(metadata.content_scripts[1].fn_name, "inject_specific");
        assert_eq!(
            metadata.content_scripts[1].matches,
            vec!["https://example.com/*", "https://test.com/*"]
        );
    }

    #[test]
    fn test_parse_content_script_with_options() {
        let source = r#"
            #[oxichrome::extension(
                name = "Test",
                version = "1.0.0"
            )]
            struct MyExt;

            #[oxichrome::content_script(
                matches = ["<all_urls>"],
                run_at = DocumentStart,
                all_frames = true,
                css = ["styles.css", "theme.css"]
            )]
            async fn inject() {}
        "#;

        let metadata = parse_source_str(source).unwrap();
        assert_eq!(metadata.content_scripts.len(), 1);
        let cs = &metadata.content_scripts[0];
        assert_eq!(cs.fn_name, "inject");
        assert_eq!(cs.matches, vec!["<all_urls>"]);
        assert_eq!(cs.run_at.as_deref(), Some("document_start"));
        assert_eq!(cs.all_frames, Some(true));
        assert_eq!(cs.css, vec!["styles.css", "theme.css"]);
    }

    #[test]
    fn test_parse_extension_with_host_permissions() {
        let source = r#"
            #[oxichrome::extension(
                name = "Test",
                version = "1.0.0",
                permissions = ["storage"],
                host_permissions = ["*://developer.mozilla.org/*", "*://*.example.org/*"],
                gecko_id = "my-ext@example.com"
            )]
            struct MyExt;
        "#;

        let metadata = parse_source_str(source).unwrap();
        assert_eq!(metadata.host_permissions.len(), 2);
        assert_eq!(metadata.host_permissions[0], "*://developer.mozilla.org/*");
        assert_eq!(metadata.host_permissions[1], "*://*.example.org/*");
        assert_eq!(metadata.gecko_id.as_deref(), Some("my-ext@example.com"));
    }

    #[test]
    fn test_parse_extension_gecko_id_defaults_to_none() {
        let source = r#"
            #[oxichrome::extension(
                name = "Test",
                version = "1.0.0"
            )]
            struct MyExt;
        "#;

        let metadata = parse_source_str(source).unwrap();
        assert!(metadata.gecko_id.is_none());
    }
}
