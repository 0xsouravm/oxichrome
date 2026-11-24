//! `#[derive(ManagedSchema)]`: the JSON schema Chrome checks an administrator's settings
//! against, made from a Rust type. Each type records its own schema; one that uses another
//! type points at it with `$ref`, and the build puts the pieces together.

use proc_macro2::TokenStream;
use quote::quote;
use serde_json::{json, Map, Value};
use syn::{Attribute, Data, DeriveInput, Fields, GenericArgument, LitStr, PathArguments, Type};

use crate::metadata;

fn doc(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(pair) => match &pair.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) => Some(text.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let text = lines.join(" ").trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[derive(Default)]
struct Serde {
    rename: Option<String>,
    rename_all: Option<String>,
    default: bool,
    skip: bool,
    flatten: bool,
    other: Vec<String>,
}

fn serde_attrs(attrs: &[Attribute]) -> syn::Result<Serde> {
    let mut found = Serde::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            let name = meta.path.get_ident().map(ToString::to_string).unwrap_or_default();
            match name.as_str() {
                "rename" => found.rename = Some(meta.value()?.parse::<LitStr>()?.value()),
                "rename_all" => found.rename_all = Some(meta.value()?.parse::<LitStr>()?.value()),
                "default" => {
                    found.default = true;
                    if meta.input.peek(syn::Token![=]) {
                        let _: LitStr = meta.value()?.parse()?;
                    }
                }
                "skip" | "skip_deserializing" => found.skip = true,
                "flatten" => found.flatten = true,
                _ => {
                    found.other.push(name);
                    if meta.input.peek(syn::Token![=]) {
                        let _: syn::Expr = meta.value()?.parse()?;
                    } else if meta.input.peek(syn::token::Paren) {
                        let _ = meta.input.parse::<proc_macro2::Group>();
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok(found)
}

fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in name.chars() {
        if c == '_' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else if c.is_uppercase() && !current.is_empty() && !current.ends_with(char::is_uppercase) {
            words.push(std::mem::take(&mut current));
            current.push(c);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.into_iter().map(|word| word.to_lowercase()).collect()
}

fn renamed(name: &str, rule: Option<&str>) -> syn::Result<String> {
    let words = words(name);
    let capital = |word: &String| word[..1].to_uppercase() + &word[1..];
    Ok(match rule {
        None => name.to_string(),
        Some("lowercase") => name.to_lowercase(),
        Some("UPPERCASE") => name.to_uppercase(),
        Some("snake_case") => words.join("_"),
        Some("SCREAMING_SNAKE_CASE") => words.join("_").to_uppercase(),
        Some("kebab-case") => words.join("-"),
        Some("SCREAMING-KEBAB-CASE") => words.join("-").to_uppercase(),
        Some("camelCase") => words.iter().enumerate().map(|(i, word)| if i == 0 { word.clone() } else { capital(word) }).collect(),
        Some("PascalCase") => words.iter().map(capital).collect(),
        Some(other) => {
            return Err(syn::Error::new(proc_macro2::Span::call_site(), format!("`ManagedSchema` doesn't know serde's `rename_all = \"{other}\"`")))
        }
    })
}

fn last_segment(ty: &Type) -> Option<(&syn::Ident, Vec<&Type>)> {
    let Type::Path(path) = ty else { return None };
    if path.qself.is_some() {
        return None;
    }
    let segment = path.path.segments.last()?;
    let args = match &segment.arguments {
        PathArguments::AngleBracketed(args) => args.args.iter().filter_map(|arg| if let GenericArgument::Type(ty) = arg { Some(ty) } else { None }).collect(),
        _ => Vec::new(),
    };
    Some((&segment.ident, args))
}

/// The schema for a field's type, and whether the field may be left out.
fn type_schema(ty: &Type) -> syn::Result<(Value, bool)> {
    let unsupported = || {
        syn::Error::new_spanned(
            ty,
            "`ManagedSchema` fields can be `bool`, integers, `f32`/`f64`, `String`, `Vec<T>`, `Option<T>`, `HashMap<String, T>`/`BTreeMap<String, T>`, or another type that derives `ManagedSchema`",
        )
    };
    if let Type::Reference(reference) = ty {
        if let Type::Path(path) = &*reference.elem {
            if path.path.is_ident("str") {
                return Ok((json!({ "type": "string" }), false));
            }
        }
        return Err(unsupported());
    }
    let (ident, args) = last_segment(ty).ok_or_else(unsupported)?;
    let name = ident.to_string();
    let schema = match name.as_str() {
        "bool" => json!({ "type": "boolean" }),
        "u8" | "u16" | "u32" | "u64" | "usize" => json!({ "type": "integer", "minimum": 0 }),
        "i8" | "i16" | "i32" | "i64" | "isize" => json!({ "type": "integer" }),
        "f32" | "f64" => json!({ "type": "number" }),
        "String" => json!({ "type": "string" }),
        "Option" => {
            let inner = args.first().ok_or_else(unsupported)?;
            return Ok((type_schema(inner)?.0, true));
        }
        "Vec" => {
            let inner = args.first().ok_or_else(unsupported)?;
            json!({ "type": "array", "items": type_schema(inner)?.0 })
        }
        "HashMap" | "BTreeMap" => {
            let (Some(key), Some(value)) = (args.first(), args.get(1)) else { return Err(unsupported()) };
            if !matches!(last_segment(key), Some((ident, _)) if ident == "String") {
                return Err(syn::Error::new_spanned(key, "a managed map's keys are strings; use `String`"));
            }
            json!({ "type": "object", "additionalProperties": type_schema(value)?.0 })
        }
        "char" | "u128" | "i128" | "Box" | "Rc" | "Arc" | "HashSet" | "BTreeSet" => return Err(unsupported()),
        _ if args.is_empty() => json!({ "$ref": name }),
        _ => return Err(unsupported()),
    };
    Ok((schema, false))
}

fn schema_of(input: &DeriveInput) -> syn::Result<Value> {
    let container = serde_attrs(&input.attrs)?;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&input.generics, "`ManagedSchema` types can't be generic"));
    }
    let mut schema = match &input.data {
        Data::Struct(data) => {
            let Fields::Named(fields) = &data.fields else {
                return Err(syn::Error::new_spanned(&input.ident, "`ManagedSchema` needs a struct with named fields (each one a policy setting) or an enum of unit variants"));
            };
            let mut properties = Map::new();
            for field in &fields.named {
                let attrs = serde_attrs(&field.attrs)?;
                if attrs.skip {
                    continue;
                }
                if attrs.flatten {
                    return Err(syn::Error::new_spanned(field, "`ManagedSchema` can't follow `#[serde(flatten)]`; nest the type as a field instead"));
                }
                let ident = field.ident.as_ref().expect("named field").to_string();
                let ident = ident.trim_start_matches("r#");
                let key = match attrs.rename {
                    Some(name) => name,
                    None => renamed(ident, container.rename_all.as_deref())?,
                };
                let (mut property, _) = type_schema(&field.ty)?;
                if let (Some(text), Value::Object(map)) = (doc(&field.attrs), &mut property) {
                    if !map.contains_key("$ref") {
                        map.insert("description".into(), json!(text));
                    }
                }
                properties.insert(key, property);
            }
            json!({ "type": "object", "properties": properties })
        }
        Data::Enum(data) => {
            let mut names = Vec::new();
            for variant in &data.variants {
                if !matches!(variant.fields, Fields::Unit) {
                    return Err(syn::Error::new_spanned(variant, "a `ManagedSchema` enum is a choice of names, so its variants can't hold data"));
                }
                let attrs = serde_attrs(&variant.attrs)?;
                names.push(match attrs.rename {
                    Some(name) => name,
                    None => renamed(&variant.ident.to_string(), container.rename_all.as_deref())?,
                });
            }
            if names.is_empty() {
                return Err(syn::Error::new_spanned(&input.ident, "a `ManagedSchema` enum needs at least one variant"));
            }
            json!({ "type": "string", "enum": names })
        }
        Data::Union(_) => return Err(syn::Error::new_spanned(&input.ident, "`ManagedSchema` can't describe a union")),
    };
    if let (Some(text), Value::Object(map)) = (doc(&input.attrs), &mut schema) {
        map.insert("description".into(), json!(text));
    }
    Ok(schema)
}

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    let schema = schema_of(&input)?;
    let ident = &input.ident;
    let entry = metadata::emit(json!({ "kind": "managed_schema", "type": ident.to_string(), "schema": schema }));
    Ok(quote! {
        #entry

        impl oxichrome::core::storage::managed::ManagedSchema for #ident {}

    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(source: &str) -> Value {
        schema_of(&syn::parse_str(source).unwrap()).unwrap()
    }

    #[test]
    fn structs_become_objects() {
        let value = schema(
            r#"
            /// What the IT team sets.
            #[serde(rename_all = "camelCase")]
            struct Policy {
                /// The server to sync with.
                server_url: String,
                max_tabs: Option<u32>,
                offset: i64,
                ratio: f64,
                locked: bool,
                #[serde(rename = "Blocked")]
                blocked_sites: Vec<String>,
                limits: std::collections::HashMap<String, u16>,
                theme: Theme,
                #[serde(skip)]
                cache: String,
            }"#,
        );
        assert_eq!(
            value,
            json!({
                "type": "object",
                "description": "What the IT team sets.",
                "properties": {
                    "serverUrl": { "type": "string", "description": "The server to sync with." },
                    "maxTabs": { "type": "integer", "minimum": 0 },
                    "offset": { "type": "integer" },
                    "ratio": { "type": "number" },
                    "locked": { "type": "boolean" },
                    "Blocked": { "type": "array", "items": { "type": "string" } },
                    "limits": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 } },
                    "theme": { "$ref": "Theme" }
                }
            })
        );
    }

    #[test]
    fn enums_become_names() {
        assert_eq!(
            schema(r#"#[serde(rename_all = "kebab-case")] enum Theme { DarkBlue, Light, #[serde(rename = "sys")] System }"#),
            json!({ "type": "string", "enum": ["dark-blue", "light", "sys"] })
        );
    }

    #[test]
    fn what_chrome_can_not_describe_is_refused() {
        for source in [
            "struct A(u32);",
            "struct A { x: char }",
            "struct A { x: HashMap<u32, u32> }",
            "struct A<T> { x: T }",
            "enum A { B(u32) }",
            "struct A { #[serde(flatten)] x: B }",
            r#"#[serde(rename_all = "Title Case")] struct A { x: u32 }"#,
        ] {
            assert!(schema_of(&syn::parse_str(source).unwrap()).is_err(), "{source}");
        }
    }
}
