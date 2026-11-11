use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;

pub const MAX_ARGS: usize = 9;

#[derive(Debug, Clone)]
pub struct Message {
    pub key: String,
    pub text: String,
    pub placeholders: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Locale {
    pub name: String,
    pub file: PathBuf,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Default)]
pub struct Locales {
    pub exists: bool,
    pub locales: Vec<Locale>,
}

struct Entries<T>(Vec<(String, T)>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Entries<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for EntriesVisitor<T> {
            type Value = Entries<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, T>()? {
                    entries.push(entry);
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(EntriesVisitor(std::marker::PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMessage {
    message: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(default)]
    placeholders: Option<Entries<RawPlaceholder>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlaceholder {
    content: String,
    #[serde(default)]
    #[allow(dead_code)]
    example: Option<String>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@') && !name.starts_with("@@")
}

pub fn valid_locale(name: &str) -> bool {
    let (language, region) = match name.split_once('_') {
        Some((language, region)) => (language, Some(region)),
        None => (name, None),
    };
    let language_ok = (2..=3).contains(&language.len()) && language.chars().all(|c| c.is_ascii_lowercase());
    let region_ok = match region {
        None => true,
        Some(region) => {
            (region.len() == 2 && region.chars().all(|c| c.is_ascii_uppercase()))
                || (region.len() == 3 && region.chars().all(|c| c.is_ascii_digit()))
        }
    };
    language_ok && region_ok
}

fn locale_hint(name: &str) -> String {
    let fixed = name.replace('-', "_");
    let fixed = match fixed.split_once('_') {
        Some((language, region)) => format!("{}_{}", language.to_ascii_lowercase(), region.to_ascii_uppercase()),
        None => fixed.to_ascii_lowercase(),
    };
    if fixed != name && valid_locale(&fixed) {
        format!("; rename it to `{fixed}`")
    } else {
        "; use a language code like `en`, `de` or `pt_BR`".to_string()
    }
}

#[derive(Debug, PartialEq)]
enum Piece {
    Text,
    Position(usize),
    Named(String),
}

fn pieces(text: &str, allow_named: bool) -> Result<Vec<Piece>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            i += 1;
            continue;
        }
        let next = chars.get(i + 1).copied();
        match next {
            Some('$') => {
                found.push(Piece::Text);
                i += 2;
            }
            Some(c) if c.is_ascii_digit() => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let digits: String = chars[i + 1..j].iter().collect();
                let number: usize = digits.parse().unwrap_or(usize::MAX);
                if number == 0 || number > MAX_ARGS {
                    return Err(format!(
                        "`${digits}`: browsers only fill `$1` to `${MAX_ARGS}` (Chrome returns nothing at all past `$9`); write `$${digits}` for a dollar sign"
                    ));
                }
                found.push(Piece::Position(number));
                i = j;
            }
            Some(c) if allow_named && (c.is_ascii_alphabetic() || c == '_') => {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '@') {
                    j += 1;
                }
                if chars.get(j) == Some(&'$') {
                    found.push(Piece::Named(chars[i + 1..j].iter().collect()));
                    i = j + 1;
                } else {
                    i = j;
                }
            }
            _ => i += 1,
        }
    }
    Ok(found)
}

impl Message {
    pub fn arg_count(&self) -> usize {
        let mut most = 0;
        for piece in pieces(&self.text, true).unwrap_or_default() {
            match piece {
                Piece::Position(n) => most = most.max(n),
                Piece::Named(name) => {
                    if let Some((_, content)) = self.placeholders.iter().find(|(p, _)| p.eq_ignore_ascii_case(&name)) {
                        for inner in pieces(content, false).unwrap_or_default() {
                            if let Piece::Position(n) = inner {
                                most = most.max(n);
                            }
                        }
                    }
                }
                Piece::Text => {}
            }
        }
        most
    }

    pub fn named_args(&self) -> Vec<(String, usize)> {
        self.placeholders
            .iter()
            .filter_map(|(name, content)| {
                match pieces(content, false).ok()?.as_slice() {
                    [Piece::Position(n)] if content.trim() == format!("${n}") => Some((name.clone(), *n)),
                    _ => None,
                }
            })
            .collect()
    }

    fn check(&self) -> Result<(), String> {
        let key = &self.key;
        if !valid_name(key) {
            return Err(format!(
                "`{key}` isn't a valid message name: use letters, digits and `_` (browsers ignore other names, and `@@` names are reserved)"
            ));
        }
        if self.text.trim().is_empty() {
            return Err(format!("`{key}` has an empty message; browsers show nothing for it"));
        }
        for (i, (name, _)) in self.placeholders.iter().enumerate() {
            if !valid_name(name) {
                return Err(format!("`{key}`: `{name}` isn't a valid placeholder name: use letters, digits and `_`"));
            }
            if self.placeholders[..i].iter().any(|(other, _)| other.eq_ignore_ascii_case(name)) {
                return Err(format!("`{key}`: the placeholder `{name}` is listed twice (browsers ignore case in placeholder names)"));
            }
        }
        let mut positions = Vec::new();
        let mut used = Vec::new();
        for piece in pieces(&self.text, true).map_err(|err| format!("`{key}`: {err}"))? {
            match piece {
                Piece::Position(n) => positions.push(n),
                Piece::Named(name) => {
                    let Some((declared, content)) = self.placeholders.iter().find(|(p, _)| p.eq_ignore_ascii_case(&name)) else {
                        return Err(format!(
                            "`{key}` uses `${name}$`, but `placeholders` has no `{name}`; add it, or write `$$` for a dollar sign"
                        ));
                    };
                    used.push(declared.clone());
                    for inner in pieces(content, false).map_err(|err| format!("`{key}`, placeholder `{declared}`: {err}"))? {
                        if let Piece::Position(n) = inner {
                            positions.push(n);
                        }
                    }
                }
                Piece::Text => {}
            }
        }
        if let Some((unused, _)) = self.placeholders.iter().find(|(name, _)| !used.contains(name)) {
            return Err(format!("`{key}`: the placeholder `{unused}` isn't used; write `${unused}$` in the message or remove it"));
        }
        let most = positions.iter().copied().max().unwrap_or(0);
        if let Some(missing) = (1..=most).find(|n| !positions.contains(n)) {
            return Err(format!(
                "`{key}` uses `${most}` but not `${missing}`; number arguments from `$1` without gaps (write `$${most}` for a dollar sign)"
            ));
        }
        Ok(())
    }
}

pub fn dir() -> PathBuf {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Path::new(&root).join("static").join("_locales")
}

pub fn load() -> Result<Locales, String> {
    let dir = dir();
    let mut locales = Locales { exists: dir.is_dir(), locales: Vec::new() };
    if !locales.exists {
        return Ok(locales);
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|err| format!("can't read `static/_locales`: {err}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        if !valid_locale(&name) {
            return Err(format!("`static/_locales/{name}` isn't a locale name browsers accept{}", locale_hint(&name)));
        }
        let file = dir.join(&name).join("messages.json");
        let text = std::fs::read_to_string(&file).map_err(|_| {
            format!("`static/_locales/{name}/messages.json` is missing; Chrome refuses to load the extension without it")
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let raw: Entries<RawMessage> = serde_json::from_str(text)
            .map_err(|err| format!("`static/_locales/{name}/messages.json`: {err}"))?;
        let mut messages: Vec<Message> = Vec::new();
        for (key, raw) in raw.0 {
            if let Some(other) = messages.iter().find(|m| m.key.eq_ignore_ascii_case(&key)) {
                return Err(format!(
                    "`static/_locales/{name}/messages.json`: `{key}` is listed twice{}",
                    if other.key == key { String::new() } else { format!(" (as `{}`; browsers ignore case in message names)", other.key) }
                ));
            }
            let message = Message {
                key,
                text: raw.message,
                placeholders: raw.placeholders.map(|p| p.0.into_iter().map(|(n, p)| (n, p.content)).collect()).unwrap_or_default(),
            };
            message.check().map_err(|err| format!("`static/_locales/{name}/messages.json`: {err}"))?;
            messages.push(message);
        }
        locales.locales.push(Locale { name, file, messages });
    }
    Ok(locales)
}

impl Locales {
    pub fn get(&self, locale: &str) -> Option<&Locale> {
        self.locales.iter().find(|l| l.name == locale)
    }

    pub fn find(&self, key: &str) -> Vec<(&Locale, &Message)> {
        self.locales
            .iter()
            .filter_map(|locale| locale.messages.iter().find(|m| m.key == key).map(|m| (locale, m)))
            .collect()
    }

    pub fn suggest(&self, key: &str) -> Option<String> {
        let all = self.locales.iter().flat_map(|l| l.messages.iter().map(|m| m.key.as_str()));
        if let Some(same) = all.clone().find(|k| k.eq_ignore_ascii_case(key)) {
            return Some(same.to_string());
        }
        let mut close: Vec<&str> = all.filter(|k| crate::permission::distance(k, key) <= 2).collect();
        close.sort_by_key(|k| crate::permission::distance(k, key));
        close.first().map(|k| k.to_string())
    }

    pub fn files(&self) -> Vec<String> {
        self.locales.iter().map(|l| l.file.to_string_lossy().into_owned()).collect()
    }

    pub fn check_default(&self, default: &str) -> Result<(), String> {
        let Some(base) = self.get(default) else {
            let have = self.locales.iter().map(|l| format!("`{}`", l.name)).collect::<Vec<_>>().join(", ");
            return Err(if !valid_locale(default) {
                format!("`{default}` isn't a locale name browsers accept{}", locale_hint(default))
            } else if self.locales.is_empty() {
                format!("add `static/_locales/{default}/messages.json` with your messages; `default_locale` needs it")
            } else {
                format!("there's no `static/_locales/{default}/messages.json`; the locales are {have}")
            });
        };
        for locale in &self.locales {
            if locale.name == default {
                continue;
            }
            for message in &locale.messages {
                let Some(original) = base.messages.iter().find(|m| m.key == message.key) else {
                    return Err(format!(
                        "`static/_locales/{}/messages.json` has `{}`, which isn't in the default locale `{default}`; add it there (other languages fall back to it) or remove it",
                        locale.name, message.key
                    ));
                };
                if message.arg_count() > original.arg_count() {
                    return Err(format!(
                        "`{}` takes {} in `{}` but {} in `{default}`; calls pass what `{default}` asks for, so the extra `$` would show nothing",
                        message.key,
                        arguments(message.arg_count()),
                        locale.name,
                        original.arg_count()
                    ));
                }
                for (name, n) in message.named_args() {
                    if let Some((_, m)) = original.named_args().into_iter().find(|(o, _)| o.eq_ignore_ascii_case(&name)) {
                        if m != n {
                            return Err(format!(
                                "`{}`: the placeholder `{name}` is `${n}` in `{}` but `${m}` in `{default}`; keep each name on the same argument",
                                message.key, locale.name
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn arguments(n: usize) -> String {
    if n == 1 { "1 argument".to_string() } else { format!("{n} arguments") }
}

pub fn tracking(locales: &Locales) -> proc_macro2::TokenStream {
    let files = locales.files();
    quote::quote! {
        #[doc(hidden)]
        const _: () = {
            #(const _: &[u8] = include_bytes!(#files);)*
            const _: Option<&str> = option_env!("OXICHROME_LOCALES");
        };
    }
}

pub struct Key {
    pub name: String,
    pub span: proc_macro2::Span,
}

impl syn::parse::Parse for Key {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            let text: syn::LitStr = input.parse()?;
            return Ok(Key { name: text.value(), span: text.span() });
        }
        let ident: syn::Ident = syn::ext::IdentExt::parse_any(input)
            .map_err(|err| syn::Error::new(err.span(), "expected the message name, like `t!(greeting)`"))?;
        let name = ident.to_string();
        Ok(Key { name: name.strip_prefix("r#").unwrap_or(&name).to_string(), span: ident.span() })
    }
}

pub struct Resolved {
    pub locales: Locales,
    pub args: usize,
    pub named: Vec<(String, usize)>,
    pub texts: Vec<String>,
}

pub fn resolve(key: &Key) -> syn::Result<Resolved> {
    let locales = load().map_err(|err| syn::Error::new(key.span, err))?;
    if !locales.exists || locales.locales.is_empty() {
        return Err(syn::Error::new(
            key.span,
            format!("there are no messages yet: add `static/_locales/en/messages.json` with `\"{}\": {{ \"message\": \"...\" }}`, and `default_locale = \"en\"` to #[oxichrome::extension]", key.name),
        ));
    }
    let found = locales.find(&key.name);
    if found.is_empty() {
        let hint = match locales.suggest(&key.name) {
            Some(close) => format!("; did you mean `{close}`?"),
            None => format!("; add it to `static/_locales/<locale>/messages.json`"),
        };
        return Err(syn::Error::new(key.span, format!("there's no message `{}`{hint}", key.name)));
    }
    let args = found.iter().map(|(_, m)| m.arg_count()).max().unwrap_or(0);
    let mut named: Vec<(String, usize)> = Vec::new();
    for (_, message) in &found {
        for (name, n) in message.named_args() {
            if !named.iter().any(|(seen, _)| seen.eq_ignore_ascii_case(&name)) {
                named.push((name, n));
            }
        }
    }
    let texts = found.iter().map(|(_, m)| m.text.clone()).collect();
    Ok(Resolved { locales, args, named, texts })
}

pub fn message_entry(key: &str, args: usize, at: &str) -> proc_macro2::TokenStream {
    crate::metadata::emit(serde_json::json!({
        "kind": "message",
        "key": key,
        "args": args,
        "at": at,
    }))
}

#[derive(Debug, Clone)]
pub enum Text {
    Literal(syn::LitStr),
    Message { key: String, span: proc_macro2::Span, texts: Vec<String>, tracking: String },
}

impl Text {
    pub fn parse_field(input: syn::parse::ParseStream, field: &str) -> syn::Result<Text> {
        if input.peek(syn::LitStr) {
            let text: syn::LitStr = input.parse()?;
            let value = text.value();
            if let Some(key) = value.strip_prefix("__MSG_").and_then(|rest| rest.strip_suffix("__")) {
                return Err(syn::Error::new(text.span(), format!("write `{field} = t!({key})`; oxichrome checks the message exists")));
            }
            return Ok(Text::Literal(text));
        }
        let path: syn::Path = input.parse().map_err(|err| {
            syn::Error::new(err.span(), format!("`{field}` takes a string like \"...\" or a message like `t!(name)`"))
        })?;
        if path.segments.last().map(|s| s.ident.to_string()).as_deref() != Some("t") {
            return Err(syn::Error::new_spanned(&path, format!("`{field}` takes a string like \"...\" or a message like `t!(name)`")));
        }
        input.parse::<syn::Token![!]>()?;
        let content;
        syn::parenthesized!(content in input);
        let key: Key = content.parse()?;
        if !content.is_empty() {
            return Err(syn::Error::new(
                content.span(),
                format!("the browser shows `{field}` itself and can't fill in arguments; use a message without `$1`"),
            ));
        }
        let resolved = resolve(&key)?;
        if resolved.args > 0 {
            return Err(syn::Error::new(
                key.span,
                format!("`{}` takes {}, but the browser shows `{field}` itself and can't fill them in; use a message without `$1`", key.name, arguments(resolved.args)),
            ));
        }
        Ok(Text::Message {
            key: key.name,
            span: key.span,
            texts: resolved.texts,
            tracking: tracking(&resolved.locales).to_string(),
        })
    }

    pub fn value(&self) -> String {
        match self {
            Text::Literal(text) => text.value(),
            Text::Message { key, .. } => format!("__MSG_{key}__"),
        }
    }

    pub fn texts(&self) -> Vec<String> {
        match self {
            Text::Literal(text) => vec![text.value()],
            Text::Message { texts, .. } => texts.clone(),
        }
    }

    pub fn span(&self) -> proc_macro2::Span {
        match self {
            Text::Literal(text) => text.span(),
            Text::Message { span, .. } => *span,
        }
    }

    pub fn checks(&self, at: &str) -> proc_macro2::TokenStream {
        match self {
            Text::Literal(_) => proc_macro2::TokenStream::new(),
            Text::Message { key, tracking, .. } => {
                let tracking: proc_macro2::TokenStream = tracking.parse().unwrap_or_default();
                let entry = message_entry(key, 0, at);
                quote::quote! { #tracking #entry }
            }
        }
    }
}

pub fn expand_t(input: proc_macro2::TokenStream, at: &str) -> syn::Result<proc_macro2::TokenStream> {
    use syn::parse::Parser;
    let parser = |input: syn::parse::ParseStream| -> syn::Result<(Key, Vec<(Option<syn::Ident>, syn::Expr)>)> {
        let key: Key = input.parse()?;
        let mut args = Vec::new();
        while !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            if input.peek(syn::Ident) && input.peek2(syn::Token![=]) && !input.peek2(syn::Token![==]) {
                let name: syn::Ident = syn::ext::IdentExt::parse_any(input)?;
                input.parse::<syn::Token![=]>()?;
                args.push((Some(name), input.parse()?));
            } else {
                args.push((None, input.parse()?));
            }
        }
        Ok((key, args))
    };
    let (key, args) = parser.parse2(input)?;
    let resolved = resolve(&key)?;
    let want = resolved.args;
    let name = &key.name;
    let named_count = args.iter().filter(|(n, _)| n.is_some()).count();
    let ordered: Vec<syn::Expr> = if named_count == 0 {
        if args.len() != want {
            let span = args.get(want).map(|(_, e)| syn::spanned::Spanned::span(e)).unwrap_or(key.span);
            let list = (1..=want).map(|n| format!("`${n}`")).collect::<Vec<_>>().join(", ");
            return Err(syn::Error::new(
                span,
                if want == 0 {
                    format!("`{name}` takes no arguments; its message has no `$1`")
                } else {
                    format!("`{name}` takes {} ({list}), but {} given", arguments(want), if args.len() == 1 { "1 was".to_string() } else { format!("{} were", args.len()) })
                },
            ));
        }
        args.into_iter().map(|(_, e)| e).collect()
    } else {
        if named_count != args.len() {
            let (_, first) = args.iter().find(|(n, _)| n.is_none()).unwrap();
            return Err(syn::Error::new_spanned(first, "name every argument or none, like `t!(greeting, who = name)`"));
        }
        let mut slots: Vec<Option<syn::Expr>> = vec![None; want];
        for (arg_name, expr) in args {
            let arg_name = arg_name.unwrap();
            let text = arg_name.to_string();
            let text = text.strip_prefix("r#").unwrap_or(&text).to_string();
            let Some((_, n)) = resolved.named.iter().find(|(p, _)| p.eq_ignore_ascii_case(&text)) else {
                let have = resolved.named.iter().map(|(p, _)| format!("`{p}`")).collect::<Vec<_>>().join(", ");
                return Err(syn::Error::new(
                    arg_name.span(),
                    if have.is_empty() {
                        format!("`{name}` has no named placeholders; pass its arguments in order")
                    } else {
                        format!("`{name}` has no placeholder `{text}`; it has {have}")
                    },
                ));
            };
            if slots[n - 1].is_some() {
                return Err(syn::Error::new(arg_name.span(), format!("`{text}` is given twice")));
            }
            slots[n - 1] = Some(expr);
        }
        if let Some(missing) = slots.iter().position(Option::is_none) {
            let label = resolved
                .named
                .iter()
                .find(|(_, n)| *n == missing + 1)
                .map(|(p, _)| format!("`{p}`"))
                .unwrap_or_else(|| format!("`${}` (it has no name, so pass the arguments in order)", missing + 1));
            return Err(syn::Error::new(key.span, format!("`{name}` also needs {label}")));
        }
        slots.into_iter().map(Option::unwrap).collect()
    };
    let tracking = tracking(&resolved.locales);
    let entry = message_entry(name, want, at);
    Ok(quote::quote! {
        {
            #tracking
            #entry
            oxichrome::i18n::__message(#name, &[#(::std::string::ToString::to_string(&(#ordered))),*])
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str, placeholders: &[(&str, &str)]) -> Message {
        Message {
            key: "k".to_string(),
            text: text.to_string(),
            placeholders: placeholders.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        }
    }

    #[test]
    fn counts_arguments() {
        assert_eq!(message("Hello $1, you are $2", &[]).arg_count(), 2);
        assert_eq!(message("Hi $who$ from $where$", &[("who", "$1"), ("where", "$2")]).arg_count(), 2);
        assert_eq!(message("costs $$5", &[]).arg_count(), 0);
        assert_eq!(message("$X$ end", &[("x", "fixed")]).arg_count(), 0);
        assert_eq!(message("Hi $who$", &[("who", "$1")]).named_args(), vec![("who".to_string(), 1)]);
        assert!(message("Hi $who$", &[("who", "Dr. $1")]).named_args().is_empty());
    }

    #[test]
    fn rejects_bad_messages() {
        let err = |text: &str, placeholders: &[(&str, &str)]| message(text, placeholders).check().unwrap_err();
        assert!(err("$1 $2 $3 $4 $5 $6 $7 $8 $9 $10", &[]).contains("only fill `$1` to `$9`"));
        assert!(err("costs $5", &[]).contains("uses `$5` but not `$1`"));
        assert!(err("Hi $who$", &[]).contains("has no `who`"));
        assert!(err("Hi", &[("who", "$1")]).contains("`who` isn't used"));
        assert!(err("Hi $a$", &[("a", "$1"), ("A", "$2")]).contains("listed twice"));
        assert!(err(" ", &[]).contains("empty"));
        assert!(message("Hi $1 $$ and $ alone", &[]).check().is_ok());
        let bad = Message { key: "bad-key".to_string(), text: "x".to_string(), placeholders: vec![] };
        assert!(bad.check().unwrap_err().contains("valid message name"));
        let reserved = Message { key: "@@ui_locale".to_string(), text: "x".to_string(), placeholders: vec![] };
        assert!(reserved.check().is_err());
    }

    #[test]
    fn locale_names() {
        for good in ["en", "pt_BR", "es_419", "fil"] {
            assert!(valid_locale(good), "{good}");
        }
        for bad in ["pt-BR", "EN", "en_us", "english", "e"] {
            assert!(!valid_locale(bad), "{bad}");
        }
        assert_eq!(locale_hint("pt-BR"), "; rename it to `pt_BR`");
        assert_eq!(locale_hint("en_us"), "; rename it to `en_US`");
    }
}
