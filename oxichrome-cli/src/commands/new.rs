use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use oxichrome_build::templates::{self, Parts};

pub fn run(path: &str, parts: &Parts) -> Result<()> {
    let dir = Path::new(path);
    let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
        bail!("`{path}` doesn't end in a folder name to use for the extension");
    };
    templates::check_name(name).map_err(anyhow::Error::msg)?;
    if dir.is_file() {
        bail!("`{path}` is a file, not a folder");
    }
    if dir.is_dir() && fs::read_dir(dir).with_context(|| format!("couldn't read `{path}`"))?.next().is_some() {
        bail!("`{path}` already exists and isn't empty; pick a new folder name");
    }

    for (file, bytes) in templates::project(name, parts) {
        let target = dir.join(&file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("couldn't create `{}`", parent.display()))?;
        }
        fs::write(&target, bytes).with_context(|| format!("couldn't write `{}`", target.display()))?;
    }

    println!("Created {} ({}) in {path}", templates::display_name(name), parts.describe());
    println!();
    println!("  cd {path}");
    println!("  cargo oxichrome build");
    println!();
    println!("Then load dist/chromium in chrome://extensions with Developer mode on (Load unpacked).");
    println!("README.md has the Firefox steps and a map of the examples.");
    Ok(())
}
