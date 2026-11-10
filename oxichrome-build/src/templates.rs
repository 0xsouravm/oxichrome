fn oxichrome_requirement() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_string(),
    }
}

pub fn cargo_toml(name: &str) -> String {
    let oxichrome = oxichrome_requirement();
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
oxichrome = {{ version = "{oxichrome}" }}
wasm-bindgen = "0.2"
serde = {{ version = "1", features = ["derive"] }}
"#
    )
}

pub fn lib_rs(name: &str) -> String {
    // kebab-case to Title Case
    let display_name: String = name
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r#"use oxichrome::prelude::*;

#[oxichrome::extension(
    name = "{display_name}",
    version = "0.1.0",
    permissions = ["storage"]
)]
struct Extension;

#[oxichrome::background]
async fn start() {{
    oxichrome::log!("{display_name} started!");
}}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {{
    oxichrome::log!("{display_name} installed: {{:?}}", details);
}}
"#
    )
}

pub fn gitignore() -> &'static str {
    r#"/target
/dist
*.wasm
Cargo.lock
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_toml_depends_on_this_oxichrome_release() {
        let version = env!("CARGO_PKG_VERSION");
        let minor = version.rsplit_once('.').unwrap().0;
        assert!(cargo_toml("x").contains(&format!("oxichrome = {{ version = \"{minor}\" }}")));
    }
}
