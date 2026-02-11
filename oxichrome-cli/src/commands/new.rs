use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use oxichrome_build::templates;

pub fn run(name: &str) -> Result<()> {
    let project_dir = Path::new(name);

    if project_dir.exists() {
        anyhow::bail!("directory `{name}` already exists");
    }

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).context("failed to create project directories")?;

    fs::write(project_dir.join("Cargo.toml"), templates::cargo_toml(name))
        .context("failed to write Cargo.toml")?;

    fs::write(src_dir.join("lib.rs"), templates::lib_rs(name))
        .context("failed to write src/lib.rs")?;

    fs::write(project_dir.join(".gitignore"), templates::gitignore())
        .context("failed to write .gitignore")?;

    println!("Created new oxichrome extension: {name}");
    println!();
    println!("  cd {name}");
    println!("  cargo oxichrome build");
    println!();
    println!("Then load the `dist/` folder as an unpacked extension in Chrome.");

    Ok(())
}
