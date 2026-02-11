use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use oxichrome_build::{manifest, shims, source_parser};

pub fn run(release: bool) -> Result<()> {
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

    let target_dir_for_lock = get_target_dir()?;
    let workspace_root = target_dir_for_lock.parent().unwrap_or(&crate_dir);
    ensure_wasm_bindgen_cli(workspace_root, &crate_dir)?;

    let profile = if release { "release" } else { "debug" };
    println!("[oxichrome] Running cargo build ({profile})...");
    let mut cargo_args = vec![
        "build",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ];
    if release {
        cargo_args.push("--release");
    }

    let status = Command::new("cargo")
        .args(&cargo_args)
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

    let dist_dir = crate_dir.join("dist");
    let wasm_dist_dir = dist_dir.join("wasm");
    fs::create_dir_all(&wasm_dist_dir).context("failed to create dist/wasm directory")?;

    println!("[oxichrome] Running wasm-bindgen...");
    let status = Command::new("wasm-bindgen")
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

    let src_lib = find_lib_rs(&crate_dir)?;
    println!("[oxichrome] Parsing {}...", src_lib.display());
    let metadata = source_parser::parse_source(&src_lib)
        .context("failed to parse extension source")?;

    let manifest_json = manifest::generate_manifest(&metadata)?;
    fs::write(dist_dir.join("manifest.json"), &manifest_json)
        .context("failed to write manifest.json")?;
    println!("[oxichrome] Generated manifest.json");

    let background_js = shims::generate_background_js(&metadata, &crate_name);
    fs::write(dist_dir.join("background.js"), &background_js)
        .context("failed to write background.js")?;
    println!("[oxichrome] Generated background.js");

    if metadata.has_popup {
        let popup_html = shims::generate_popup_html();
        fs::write(dist_dir.join("popup.html"), &popup_html)
            .context("failed to write popup.html")?;
        let popup_js = shims::generate_popup_js(&crate_name);
        fs::write(dist_dir.join("popup.js"), &popup_js)
            .context("failed to write popup.js")?;
        println!("[oxichrome] Generated popup.html + popup.js");
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

    let static_dir = crate_dir.join("static");
    if static_dir.is_dir() {
        println!("[oxichrome] Copying static assets...");
        copy_dir_recursive(&static_dir, &dist_dir)
            .context("failed to copy static assets")?;
    }

    let wasm_output = wasm_dist_dir.join(format!("{wasm_name}_bg.wasm"));
    if which_exists("wasm-opt") {
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
    println!("[oxichrome] Load the dist/ folder as an unpacked extension in Chrome/Brave.");

    Ok(())
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

fn ensure_wasm_bindgen_cli(workspace_root: &Path, crate_dir: &Path) -> Result<()> {
    let desired_version = read_wasm_bindgen_version(workspace_root)
        .or_else(|| read_wasm_bindgen_version(crate_dir));

    let output = Command::new("wasm-bindgen").arg("--version").output();
    if let Ok(output) = output {
        let version_str = String::from_utf8_lossy(&output.stdout);
        if let Some(desired) = &desired_version {
            if version_str.contains(desired) {
                return Ok(());
            }
            println!(
                "[oxichrome] wasm-bindgen-cli version mismatch (want {desired}). Reinstalling..."
            );
        } else {
            return Ok(());
        }
    }

    let mut args = vec!["install", "wasm-bindgen-cli"];
    let version_flag;
    if let Some(version) = &desired_version {
        version_flag = format!("--version={version}");
        args.push(&version_flag);
    }

    println!("[oxichrome] Installing wasm-bindgen-cli...");
    let status = Command::new("cargo")
        .args(&args)
        .status()
        .context("failed to install wasm-bindgen-cli")?;

    if !status.success() {
        bail!("failed to install wasm-bindgen-cli");
    }

    Ok(())
}

fn read_wasm_bindgen_version(crate_dir: &Path) -> Option<String> {
    let lock_path = crate_dir.join("Cargo.lock");
    let content = fs::read_to_string(lock_path).ok()?;

    let mut in_wasm_bindgen = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_wasm_bindgen = false;
        } else if trimmed == r#"name = "wasm-bindgen""# {
            in_wasm_bindgen = true;
        } else if in_wasm_bindgen && trimmed.starts_with("version = ") {
            let version = trimmed
                .strip_prefix("version = \"")
                .and_then(|s| s.strip_suffix('"'));
            return version.map(|s| s.to_string());
        }
    }
    None
}

fn find_lib_rs(crate_dir: &Path) -> Result<PathBuf> {
    let lib_rs = crate_dir.join("src").join("lib.rs");
    if lib_rs.exists() {
        return Ok(lib_rs);
    }
    bail!("could not find src/lib.rs")
}

fn extract_crate_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") && trimmed.contains('=') {
            let value = trimmed.split('=').nth(1)?.trim();
            let name = value.trim_matches('"').trim_matches('\'');
            return Some(name.to_string());
        }
    }
    None
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

fn which_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
