use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub popup: bool,
    pub options: bool,
    pub side_panel: bool,
    pub content_script: bool,
    pub i18n: bool,
    pub minimal: bool,
}

impl Default for Parts {
    fn default() -> Self {
        Parts { popup: true, options: true, side_panel: false, content_script: false, i18n: false, minimal: false }
    }
}

impl Parts {
    pub fn minimal() -> Self {
        Parts { popup: false, options: false, ..Parts::default() }.with_minimal()
    }

    fn with_minimal(mut self) -> Self {
        self.minimal = true;
        self
    }

    fn ui(&self) -> bool {
        self.popup || self.options || self.side_panel
    }

    fn background(&self) -> bool {
        !self.minimal
    }

    fn settings(&self) -> bool {
        self.popup || self.options
    }

    fn modules(&self) -> bool {
        self.background() || self.content_script || self.ui()
    }

    fn permissions(&self) -> Vec<&'static str> {
        let mut list = Vec::new();
        if self.settings() {
            list.push("Storage");
        }
        if self.side_panel {
            list.push("SidePanel");
        }
        list
    }

    fn flag(&self, name: &str) -> bool {
        match name {
            "popup" => self.popup,
            "options" => self.options,
            "side_panel" => self.side_panel,
            "content_script" => self.content_script,
            "i18n" => self.i18n,
            "minimal" => self.minimal,
            "ui" => self.ui(),
            "background" => self.background(),
            "settings" => self.settings(),
            "modules" => self.modules(),
            "permissions" => !self.permissions().is_empty(),
            other => panic!("unknown template flag `{other}`"),
        }
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.minimal {
            parts.push("background only");
        }
        if self.popup {
            parts.push("popup");
        }
        if self.options {
            parts.push("options page");
        }
        if self.side_panel {
            parts.push("side panel");
        }
        if self.content_script {
            parts.push("content script");
        }
        if self.i18n {
            parts.push("translations");
        }
        if parts.is_empty() {
            parts.push("background");
        }
        parts.join(", ")
    }
}

const MESSAGES: &[(&str, &str)] = &[
    ("ext_name", "{display}"),
    ("ext_desc", "A browser extension built with oxichrome."),
    ("action_title", "{display}"),
    ("popup_hint", "Change me in src/popup.rs"),
    ("clicks_label", "clicks, saved in extension storage"),
    ("click_me", "Click me"),
    ("reset", "Reset"),
    ("settings", "Settings"),
    ("options_hint", "Change me in src/options.rs"),
    ("badge_label", "Show the count on the toolbar"),
    ("badge_hint", "Puts the number of clicks on the extension's button."),
    ("panel_title", "{display}"),
    ("panel_hint", "Change me in src/side_panel.rs"),
    ("panel_empty", "The side panel stays open while you browse."),
    ("content_hello", "Hello from {display}"),
];

enum Body {
    Text(&'static str),
    Bytes(&'static [u8]),
}

struct Template {
    path: &'static str,
    body: Body,
    when: fn(&Parts) -> bool,
}

fn always(_: &Parts) -> bool {
    true
}

const TEMPLATES: &[Template] = &[
    Template { path: "Cargo.toml", body: Body::Text(include_str!("../templates/Cargo.toml.tmpl")), when: always },
    Template { path: "README.md", body: Body::Text(include_str!("../templates/README.md.tmpl")), when: always },
    Template { path: ".gitignore", body: Body::Text(include_str!("../templates/gitignore")), when: always },
    Template { path: "src/lib.rs", body: Body::Text(include_str!("../templates/src/lib.rs.tmpl")), when: always },
    Template { path: "src/background.rs", body: Body::Text(include_str!("../templates/src/background.rs.tmpl")), when: Parts::background },
    Template { path: "src/content_script.rs", body: Body::Text(include_str!("../templates/src/content_script.rs.tmpl")), when: |p| p.content_script },
    Template { path: "src/options.rs", body: Body::Text(include_str!("../templates/src/options.rs.tmpl")), when: |p| p.options },
    Template { path: "src/popup.rs", body: Body::Text(include_str!("../templates/src/popup.rs.tmpl")), when: |p| p.popup },
    Template { path: "src/side_panel.rs", body: Body::Text(include_str!("../templates/src/side_panel.rs.tmpl")), when: |p| p.side_panel },
    Template { path: "static/base.css", body: Body::Text(include_str!("../templates/static/base.css")), when: Parts::ui },
    Template { path: "static/style.css", body: Body::Text(include_str!("../templates/static/style.css")), when: Parts::ui },
    Template { path: "static/content.css", body: Body::Text(include_str!("../templates/static/content.css.tmpl")), when: |p| p.content_script },
    Template { path: "static/icons/16.png", body: Body::Bytes(include_bytes!("../templates/static/icons/16.png")), when: always },
    Template { path: "static/icons/32.png", body: Body::Bytes(include_bytes!("../templates/static/icons/32.png")), when: always },
    Template { path: "static/icons/48.png", body: Body::Bytes(include_bytes!("../templates/static/icons/48.png")), when: always },
    Template { path: "static/icons/128.png", body: Body::Bytes(include_bytes!("../templates/static/icons/128.png")), when: always },
];

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "abstract", "become",
    "box", "do", "final", "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

const TAKEN: &[&str] = &["std", "core", "alloc", "proc_macro", "proc-macro", "test", "oxichrome", "leptos", "serde", "web-sys", "web_sys", "wasm-bindgen", "wasm_bindgen"];

pub fn check_name(name: &str) -> Result<(), String> {
    let Some(first) = name.chars().next() else {
        return Err("the extension needs a name, like `my-extension`".into());
    };
    if let Some(bad) = name.chars().find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_')) {
        return Err(format!("`{name}` can't be a crate name: `{bad}` isn't allowed; use letters, digits, `-` and `_`, like `my-extension`"));
    }
    if !first.is_ascii_alphabetic() {
        return Err(format!("`{name}` can't be a crate name: it has to start with a letter"));
    }
    if KEYWORDS.contains(&name) {
        return Err(format!("`{name}` is a Rust keyword, so it can't be a crate name"));
    }
    if TAKEN.iter().any(|taken| taken.eq_ignore_ascii_case(name)) {
        return Err(format!("`{name}` would clash with a crate the extension depends on; pick another name"));
    }
    Ok(())
}

fn words(name: &str) -> impl Iterator<Item = String> + '_ {
    name.split(['-', '_']).filter(|word| !word.is_empty()).map(|word| {
        let mut chars = word.chars();
        chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
    })
}

pub fn display_name(name: &str) -> String {
    words(name).collect::<Vec<_>>().join(" ")
}

fn struct_name(name: &str) -> String {
    let joined: String = words(name).collect();
    if joined == "Extension" {
        "MyExtension".into()
    } else {
        joined
    }
}

fn oxichrome_requirement() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_string(),
    }
}

struct Render<'a> {
    parts: &'a Parts,
    name: &'a str,
    display: String,
    used: Vec<&'static str>,
}

impl Render<'_> {
    fn message(&self, key: &str) -> (&'static str, String) {
        let (key, text) = MESSAGES.iter().find(|(k, _)| *k == key).unwrap_or_else(|| panic!("no template message `{key}`"));
        (key, text.replace("{display}", &self.display))
    }

    fn placeholder(&mut self, inner: &str) -> String {
        if let Some(key) = inner.strip_prefix("t:") {
            let (key, text) = self.message(key);
            self.used.push(key);
            return if self.parts.i18n { format!("t!({key})") } else { format!("{text:?}") };
        }
        if let Some(key) = inner.strip_prefix("tr:") {
            let (key, text) = self.message(key);
            self.used.push(key);
            return if self.parts.i18n { format!("&t!({key})") } else { format!("{text:?}") };
        }
        if let Some(key) = inner.strip_prefix("tv:") {
            let (key, text) = self.message(key);
            self.used.push(key);
            return if self.parts.i18n { format!("{{t!({key})}}") } else { format!("{text:?}") };
        }
        match inner {
            "name" => self.name.to_string(),
            "display" => self.display.clone(),
            "struct" => struct_name(self.name),
            "oxichrome" => oxichrome_requirement(),
            "permissions" => self.parts.permissions().join(", "),
            "settings_note" => if self.parts.settings() { " It also keeps the settings in extension storage and puts the click count on the toolbar button.".into() } else { String::new() },
            other => panic!("unknown template placeholder `{other}`"),
        }
    }

    fn text(&mut self, source: &str) -> String {
        let mut kept = String::new();
        let mut stack: Vec<bool> = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(cond) = trimmed.strip_prefix("#if ") {
                let cond = cond.trim();
                let on = match cond.strip_prefix('!') {
                    Some(flag) => !self.parts.flag(flag),
                    None => self.parts.flag(cond),
                };
                stack.push(on);
                continue;
            }
            if trimmed == "#endif" {
                stack.pop().expect("`#endif` without `#if` in a template");
                continue;
            }
            if stack.iter().all(|on| *on) {
                kept.push_str(line);
                kept.push('\n');
            }
        }
        assert!(stack.is_empty(), "a template has an `#if` without `#endif`");
        let mut out = String::new();
        let mut rest = kept.as_str();
        while let Some(start) = rest.find("{{") {
            let Some(len) = rest[start + 2..].find("}}") else { break };
            out.push_str(&rest[..start]);
            let inner = rest[start + 2..start + 2 + len].to_string();
            out.push_str(&self.placeholder(&inner));
            rest = &rest[start + 2 + len + 2..];
        }
        out.push_str(rest);
        out
    }
}

pub fn project(name: &str, parts: &Parts) -> Vec<(String, Vec<u8>)> {
    let mut render = Render { parts, name, display: display_name(name), used: Vec::new() };
    let mut files = Vec::new();
    for template in TEMPLATES.iter().filter(|template| (template.when)(parts)) {
        let bytes = match template.body {
            Body::Text(source) => render.text(source).into_bytes(),
            Body::Bytes(bytes) => bytes.to_vec(),
        };
        files.push((template.path.to_string(), bytes));
    }
    if parts.i18n {
        let mut messages = Map::new();
        for (key, _) in MESSAGES.iter().filter(|(key, _)| render.used.contains(key)) {
            let (_, text) = render.message(key);
            messages.insert(key.to_string(), serde_json::json!({ "message": text }));
        }
        let json = serde_json::to_string_pretty(&Value::Object(messages)).expect("messages are plain strings") + "\n";
        files.push(("static/_locales/en/messages.json".to_string(), json.into_bytes()));
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combos() -> Vec<Parts> {
        let mut all = Vec::new();
        for bits in 0..32u8 {
            let bit = |n: u8| bits & (1 << n) != 0;
            all.push(Parts { popup: bit(0), options: bit(1), side_panel: bit(2), content_script: bit(3), i18n: bit(4), minimal: false });
        }
        for bits in 0..4u8 {
            all.push(Parts { content_script: bits & 1 != 0, i18n: bits & 2 != 0, ..Parts::minimal() });
        }
        all
    }

    fn file<'a>(files: &'a [(String, Vec<u8>)], path: &str) -> Option<&'a str> {
        files.iter().find(|(p, _)| p == path).map(|(_, bytes)| std::str::from_utf8(bytes).unwrap())
    }

    #[test]
    fn cargo_toml_depends_on_this_oxichrome_release() {
        let version = env!("CARGO_PKG_VERSION");
        let minor = version.rsplit_once('.').unwrap().0;
        let files = project("x", &Parts::default());
        assert!(file(&files, "Cargo.toml").unwrap().contains(&format!("oxichrome = \"{minor}\"")));
    }

    #[test]
    fn every_combination_renders_cleanly() {
        for parts in combos() {
            let files = project("my-ext", &parts);
            for (path, bytes) in &files {
                let Ok(text) = std::str::from_utf8(bytes) else { continue };
                assert!(!text.contains("{{"), "{path} {parts:?} has a leftover placeholder");
                assert!(!text.lines().any(|line| line.trim_start().starts_with("#if") || line.trim() == "#endif"), "{path} {parts:?} has a leftover marker");
                assert!(!text.contains("__private"), "{path} {parts:?}");
                assert!(!text.contains('\u{2014}'), "{path} {parts:?} has an em-dash");
                assert!(!text.contains("\n\n\n"), "{path} {parts:?} has a double blank line");
                assert!(text.ends_with('\n') && !text.ends_with("\n\n"), "{path} {parts:?} should end with one newline");
            }
            let lib = file(&files, "src/lib.rs").unwrap();
            for (module, on) in [("background", parts.background()), ("content_script", parts.content_script), ("options", parts.options), ("popup", parts.popup), ("side_panel", parts.side_panel)] {
                assert_eq!(lib.contains(&format!("mod {module};")), on, "{parts:?} mod {module}");
                assert_eq!(file(&files, &format!("src/{module}.rs")).is_some(), on, "{parts:?} {module}.rs");
            }
            let manifest = file(&files, "Cargo.toml").unwrap();
            assert_eq!(manifest.contains("leptos"), parts.ui(), "{parts:?}");
            assert_eq!(manifest.contains("web-sys"), parts.content_script, "{parts:?}");
            assert_eq!(lib.contains("t!("), parts.i18n, "{parts:?}");
            assert_eq!(lib.contains("default_locale"), parts.i18n, "{parts:?}");
            if parts.i18n {
                let messages: Value = serde_json::from_str(file(&files, "static/_locales/en/messages.json").unwrap()).unwrap();
                let all: String = files.iter().filter_map(|(_, b)| std::str::from_utf8(b).ok()).collect();
                for key in messages.as_object().unwrap().keys() {
                    assert!(all.contains(&format!("t!({key})")), "{parts:?} unused message {key}");
                }
                for (key, _) in MESSAGES {
                    if all.contains(&format!("t!({key})")) {
                        assert!(messages.get(key).is_some(), "{parts:?} missing message {key}");
                    }
                }
            } else {
                assert!(file(&files, "static/_locales/en/messages.json").is_none());
            }
        }
    }

    #[test]
    fn the_default_is_popup_and_options() {
        let files = project("my-extension", &Parts::default());
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Cargo.toml",
                "README.md",
                ".gitignore",
                "src/lib.rs",
                "src/background.rs",
                "src/options.rs",
                "src/popup.rs",
                "static/base.css",
                "static/style.css",
                "static/icons/16.png",
                "static/icons/32.png",
                "static/icons/48.png",
                "static/icons/128.png"
            ]
        );
        let lib = file(&files, "src/lib.rs").unwrap();
        assert!(lib.contains("name = \"My Extension\""));
        assert!(lib.contains("pub struct MyExtension;"));
        assert!(lib.contains("permissions = [Storage],"));
    }

    #[test]
    fn minimal_is_one_file() {
        let files = project("bare", &Parts::minimal());
        let rs: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).filter(|p| p.ends_with(".rs")).collect();
        assert_eq!(rs, ["src/lib.rs"]);
        let lib = file(&files, "src/lib.rs").unwrap();
        assert!(lib.starts_with("use oxichrome::prelude::*;"));
        assert!(!lib.contains("permissions"));
        assert!(lib.contains("#[background]"));
    }

    #[test]
    fn names_are_checked() {
        assert!(check_name("my-extension").is_ok());
        assert!(check_name("tab_tidy2").is_ok());
        for bad in ["", "2fast", "my ext", "über", "fn", "test", "oxichrome", "Leptos", "a.b"] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
        assert_eq!(display_name("my_cool-ext"), "My Cool Ext");
        assert_eq!(struct_name("extension"), "MyExtension");
    }
}
