pub fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
oxichrome = {{ version = "0.1" }}
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
