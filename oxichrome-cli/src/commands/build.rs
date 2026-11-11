use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::Value;
use which::which;

use anyhow::{bail, Context, Result};
use oxichrome_build::{manifest, metadata, shims, Browser};

pub fn run(release: bool, browser: Browser) -> Result<()> {
    let crate_dir = std::env::current_dir()?;
    let cargo_toml_path = crate_dir.join("Cargo.toml");

    if !cargo_toml_path.exists() {
        bail!("no Cargo.toml found in current directory");
    }

    let cargo_toml_content = fs::read_to_string(&cargo_toml_path)?;
    let crate_name = extract_crate_name(&cargo_toml_content)
        .context("could not find `name` in [package] section of Cargo.toml")?;

    let wasm_name = crate_name.replace('-', "_");

    println!("[oxichrome] Building extension: {crate_name}");

    ensure_wasm_target()?;

    let profile = if release { "release" } else { "debug" };
    println!("[oxichrome] Running cargo build ({profile})...");
    let mut cargo_args = vec!["build", "--lib", "--target", "wasm32-unknown-unknown"];
    if release {
        cargo_args.push("--release");
    }

    let status = Command::new("cargo")
        .args(&cargo_args)
        .env("OXICHROME_LOCALES", locale_folders(&crate_dir))
        .status()
        .context("failed to run cargo build")?;

    if !status.success() {
        bail!("cargo build failed");
    }

    let target_dir = get_target_dir()?;
    let wasm_file = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(format!("{wasm_name}.wasm"));

    if !wasm_file.exists() {
        bail!("WASM file not found at {}", wasm_file.display());
    }

    let wasm_bytes = fs::read(&wasm_file)
        .with_context(|| format!("failed to read {}", wasm_file.display()))?;
    let section = metadata::read_section(&wasm_bytes)
        .with_context(|| format!("failed to read extension metadata from {}", wasm_file.display()))?;
    let metadata = metadata::parse_entries(&section)?;
    let default_keys = match &metadata.default_locale {
        Some(default) => Some(message_keys(&crate_dir, default)?),
        None => None,
    };
    metadata::check_messages(&metadata, default_keys.as_deref())?;

    let browser_dir = match browser {
        Browser::Chromium => "chromium",
        Browser::Firefox => "firefox",
    };
    let dist_dir = crate_dir.join("dist").join(browser_dir);
    let wasm_dist_dir = dist_dir.join("wasm");
    fs::create_dir_all(&wasm_dist_dir).context("failed to create dist/wasm directory")?;

    let wasm_bindgen = wasm_bindgen_cli()?;
    println!("[oxichrome] Running wasm-bindgen...");
    let status = Command::new(&wasm_bindgen)
        .args([
            "--target",
            "web",
            "--out-dir",
            wasm_dist_dir.to_str().unwrap(),
            "--out-name",
            &wasm_name,
            wasm_file.to_str().unwrap(),
        ])
        .status()
        .context("failed to run wasm-bindgen")?;

    if !status.success() {
        bail!("wasm-bindgen failed");
    }

    let wasm_output = wasm_dist_dir.join(format!("{wasm_name}_bg.wasm"));
    let bound = fs::read(&wasm_output)
        .with_context(|| format!("failed to read {}", wasm_output.display()))?;
    fs::write(&wasm_output, metadata::strip_wasm(&bound)?)
        .with_context(|| format!("failed to write {}", wasm_output.display()))?;

    let manifest_json = manifest::generate_manifest(&metadata, browser)?;
    fs::write(dist_dir.join("manifest.json"), &manifest_json)
        .context("failed to write manifest.json")?;
    println!("[oxichrome] Generated manifest.json");
    let left_out = manifest::left_out_permissions(&metadata, browser);
    if !left_out.is_empty() {
        let browser_name = match browser {
            oxichrome_build::Browser::Chromium => "Chrome",
            oxichrome_build::Browser::Firefox => "Firefox",
        };
        println!(
            "[oxichrome] Note: {browser_name} doesn't have {}, so this build leaves {} out",
            left_out.iter().map(|(_, ident)| format!("`{ident}`")).collect::<Vec<_>>().join(", "),
            if left_out.len() == 1 { "it" } else { "them" }
        );
    }

    let left_out_events = shims::left_out_events(&metadata, browser);
    if !left_out_events.is_empty() {
        println!(
            "[oxichrome] Note: Firefox doesn't have {}, so {} never {} there",
            if left_out_events.len() == 1 { "that event" } else { "those events" },
            match left_out_events.split_last() {
                Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                _ => left_out_events.join(""),
            },
            if left_out_events.len() == 1 { "runs" } else { "run" }
        );
    }
    let background_js = shims::generate_background_js(&metadata, &crate_name);
    fs::write(dist_dir.join("background.js"), &background_js)
        .context("failed to write background.js")?;
    println!("[oxichrome] Generated background.js");

    if metadata.has_popup {
        let popup_html = shims::generate_popup_html();
        fs::write(dist_dir.join("popup.html"), &popup_html)
            .context("failed to write popup.html")?;
        let popup_js = shims::generate_popup_js(&crate_name);
        fs::write(dist_dir.join("popup.js"), &popup_js).context("failed to write popup.js")?;
        println!("[oxichrome] Generated popup.html + popup.js");
    }

    if metadata.side_panel.is_some() {
        fs::write(dist_dir.join("sidepanel.html"), shims::generate_side_panel_html())
            .context("failed to write sidepanel.html")?;
        fs::write(dist_dir.join("sidepanel.js"), shims::generate_side_panel_js(&crate_name))
            .context("failed to write sidepanel.js")?;
        println!("[oxichrome] Generated sidepanel.html + sidepanel.js");
    }

    if metadata.has_options_page {
        let options_html = shims::generate_options_html();
        fs::write(dist_dir.join("options.html"), &options_html)
            .context("failed to write options.html")?;
        let options_js = shims::generate_options_js(&crate_name);
        fs::write(dist_dir.join("options.js"), &options_js)
            .context("failed to write options.js")?;
        println!("[oxichrome] Generated options.html + options.js");
    }

    for cs in &metadata.content_scripts {
        let cs_js = shims::generate_content_script_js(&cs.fn_name, &cs.export, &crate_name);
        let cs_filename = cs.file_name();
        fs::write(dist_dir.join(&cs_filename), &cs_js)
            .with_context(|| format!("failed to write {cs_filename}"))?;
        println!("[oxichrome] Generated {cs_filename}");
    }

    let stale_locales = dist_dir.join("_locales");
    if stale_locales.exists() {
        fs::remove_dir_all(&stale_locales).context("failed to clear dist/_locales")?;
    }
    let static_dir = crate_dir.join("static");
    if static_dir.is_dir() {
        println!("[oxichrome] Copying static assets...");
        copy_dir_recursive(&static_dir, &dist_dir).context("failed to copy static assets")?;
    }

    if command_in_path("wasm-opt") {
        println!("[oxichrome] Running wasm-opt...");
        let status = Command::new("wasm-opt")
            .args([
                "-Oz",
                wasm_output.to_str().unwrap(),
                "-o",
                wasm_output.to_str().unwrap(),
            ])
            .status();
        match status {
            Ok(s) if s.success() => println!("[oxichrome] wasm-opt optimization complete."),
            _ => println!("[oxichrome] wasm-opt failed (non-critical, continuing)."),
        }
    } else {
        println!("[oxichrome] wasm-opt not found, skipping optimization.");
    }

    println!("[oxichrome] Build complete! Output: {}", dist_dir.display());
    match browser {
        Browser::Chromium => println!("[oxichrome] Load the dist/chromium/ folder as an unpacked extension in Chrome/Brave."),
        Browser::Firefox => println!("[oxichrome] Load the dist/firefox/ folder as a temporary extension in Firefox (about:debugging)."),
    }

    Ok(())
}

fn locale_folders(crate_dir: &Path) -> String {
    let Ok(entries) = fs::read_dir(crate_dir.join("static").join("_locales")) else {
        return String::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let has_file = entry.path().join("messages.json").is_file();
            format!("{}{}", entry.file_name().to_string_lossy(), if has_file { "" } else { "?" })
        })
        .collect();
    names.sort();
    names.join(",")
}

fn message_keys(crate_dir: &Path, locale: &str) -> Result<Vec<String>> {
    let file = crate_dir.join("static").join("_locales").join(locale).join("messages.json");
    let text = fs::read_to_string(&file).with_context(|| format!("failed to read {}", file.display()))?;
    let messages: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .with_context(|| format!("{} isn't a JSON object", file.display()))?;
    Ok(messages.keys().cloned().collect())
}

fn ensure_wasm_target() -> Result<()> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("failed to run rustup")?;

    let installed = String::from_utf8_lossy(&output.stdout);
    if !installed.contains("wasm32-unknown-unknown") {
        println!("[oxichrome] Installing wasm32-unknown-unknown target...");
        let status = Command::new("rustup")
            .args(["target", "add", "wasm32-unknown-unknown"])
            .status()
            .context("failed to install wasm32-unknown-unknown")?;
        if !status.success() {
            bail!("failed to install wasm32-unknown-unknown target");
        }
    }
    Ok(())
}

fn wasm_bindgen_cli() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .output()
        .context("failed to run cargo metadata")?;
    if !output.status.success() {
        bail!("cargo metadata failed");
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata output")?;
    let version = wasm_bindgen_version(&metadata)?;

    if let Ok(on_path) = which("wasm-bindgen") {
        if reports_version(&on_path, &version) {
            return Ok(on_path);
        }
    }

    let root = cache_dir()?.join("wasm-bindgen-cli").join(&version);
    let binary = root
        .join("bin")
        .join(format!("wasm-bindgen{}", std::env::consts::EXE_SUFFIX));
    if reports_version(&binary, &version) {
        return Ok(binary);
    }

    println!(
        "[oxichrome] Installing wasm-bindgen-cli {version} into {} (once per version)...",
        root.display()
    );
    let status = Command::new("cargo")
        .args(["install", "wasm-bindgen-cli", "--locked", "--version"])
        .arg(format!("={version}"))
        .arg("--root")
        .arg(&root)
        .status()
        .context("failed to run cargo install")?;
    if !status.success() || !reports_version(&binary, &version) {
        bail!("failed to install wasm-bindgen-cli {version}");
    }
    Ok(binary)
}

fn wasm_bindgen_version(metadata: &serde_json::Value) -> Result<String> {
    let mut versions: Vec<&str> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| package["name"] == "wasm-bindgen")
        .filter_map(|package| package["version"].as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    match versions.as_slice() {
        [version] => Ok(version.to_string()),
        [] => bail!("wasm-bindgen is not a dependency of this crate; add `wasm-bindgen` to [dependencies]"),
        _ => bail!(
            "the dependency graph has several wasm-bindgen versions ({}); run `cargo update -p wasm-bindgen` to unify them",
            versions.join(", ")
        ),
    }
}

fn reports_version(binary: &Path, version: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .map(|output| parse_cli_version(&String::from_utf8_lossy(&output.stdout)) == Some(version))
        .unwrap_or(false)
}

fn parse_cli_version(output: &str) -> Option<&str> {
    let mut words = output.split_whitespace();
    (words.next()? == "wasm-bindgen").then_some(())?;
    words.next()
}

fn cache_dir() -> Result<PathBuf> {
    let env = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    if let Some(dir) = env("OXICHROME_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if cfg!(windows) {
        if let Some(dir) = env("LOCALAPPDATA") {
            return Ok(PathBuf::from(dir).join("oxichrome"));
        }
    }
    if let Some(dir) = env("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(dir).join("oxichrome"));
    }
    let home = env("HOME")
        .or_else(|| env("USERPROFILE"))
        .context("could not find a cache directory; set OXICHROME_CACHE_DIR")?;
    Ok(PathBuf::from(home).join(".cache").join("oxichrome"))
}

fn extract_crate_name(content: &str) -> Option<String> {
    let parsed: Value = toml::from_str(content).ok()?;

    parsed
        .get("package")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}

fn get_target_dir() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .context("failed to run cargo metadata")?;

    if !output.status.success() {
        bail!("cargo metadata failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).context("failed to parse cargo metadata output")?;

    let target_dir = parsed["target_directory"]
        .as_str()
        .context("cargo metadata missing target_directory")?;

    Ok(PathBuf::from(target_dir))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).context("failed to read static directory")? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn command_in_path(cmd: &str) -> bool {
    which(cmd).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_crate_name_basic() {
        let content = r#"
            [package]
            name = "my-extension"
            version = "0.1.0"
        "#;
        assert_eq!(extract_crate_name(content), Some("my-extension".into()));
    }

    #[test]
    fn test_extract_crate_name_single_quotes() {
        let content = r#"
            [package]
            name = 'weird-name_with-dashes'
        "#;
        assert_eq!(
            extract_crate_name(content),
            Some("weird-name_with-dashes".into())
        );
    }

    #[test]
    fn test_extract_crate_name_not_found() {
        let content = r#"[package] version = "0.1.0""#;
        assert_eq!(extract_crate_name(content), None);
    }

    #[test]
    fn test_extract_crate_name_ignores_other_sections() {
        let content = r#"
            [dependencies]
            anyhow = "1"

            [package]
            name = "test-crate"
        "#;
        assert_eq!(extract_crate_name(content), Some("test-crate".into()));
    }

    #[test]
    fn test_extract_crate_name_does_not_pick_wrong_section() {
        let content = r#"
            [dependencies]
            name-anyhow = "1"

            [package]
            name = "test-crate"
        "#;
        assert_eq!(extract_crate_name(content), Some("test-crate".into()));
    }

    #[test]
    fn test_parse_cli_version_is_exact() {
        assert_eq!(parse_cli_version("wasm-bindgen 0.2.108\n"), Some("0.2.108"));
        assert_eq!(
            parse_cli_version("wasm-bindgen 0.2.108 (abc123)"),
            Some("0.2.108")
        );
        assert_ne!(parse_cli_version("wasm-bindgen 0.2.108"), Some("0.2.1"));
        assert_eq!(parse_cli_version("something 0.2.108"), None);
        assert_eq!(parse_cli_version(""), None);
    }

    #[test]
    fn test_wasm_bindgen_version_from_metadata() {
        let one = serde_json::json!({ "packages": [
            { "name": "wasm-bindgen", "version": "0.2.130" },
            { "name": "wasm-bindgen-futures", "version": "0.4.1" },
            { "name": "wasm-bindgen", "version": "0.2.130" }
        ]});
        assert_eq!(wasm_bindgen_version(&one).unwrap(), "0.2.130");
        let none = serde_json::json!({ "packages": [{ "name": "serde", "version": "1.0.0" }] });
        assert!(wasm_bindgen_version(&none).is_err());
        let two = serde_json::json!({ "packages": [
            { "name": "wasm-bindgen", "version": "0.2.100" },
            { "name": "wasm-bindgen", "version": "0.2.130" }
        ]});
        assert!(wasm_bindgen_version(&two)
            .unwrap_err()
            .to_string()
            .contains("0.2.100, 0.2.130"));
    }

    #[test]
    fn test_command_in_path_exist() {
        assert!(command_in_path("cargo"));
    }

    #[test]
    fn test_command_in_path_dont_exist() {
        assert!(!command_in_path("nonexistent-command"));
    }
}
