use std::fs;

use anyhow::{bail, Result};

pub fn run() -> Result<()> {
    let crate_dir = std::env::current_dir()?;
    let cargo_toml_path = crate_dir.join("Cargo.toml");

    if !cargo_toml_path.exists() {
        bail!("No Cargo.toml found in current directory");
    }

    let dist_dir = crate_dir.join("dist");

    if !dist_dir.exists() {
        println!("[oxichrome] Nothing to clean — dist/ does not exist.");
        return Ok(());
    }

    fs::remove_dir_all(&dist_dir)?;
    println!("[oxichrome] Removed {}", dist_dir.display());

    Ok(())
}
