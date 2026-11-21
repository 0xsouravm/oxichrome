use clap::{Parser, Subcommand};
use oxichrome_build::templates::Parts;
use oxichrome_build::Browser;

mod commands;

#[derive(Parser)]
#[command(name = "cargo-oxichrome", bin_name = "cargo", version)]
struct Cli {
    #[command(subcommand)]
    command: CargoSubcommand,
}

#[derive(Subcommand)]
enum CargoSubcommand {
    Oxichrome(OxichromeArgs),
}

#[derive(Parser)]
#[command(version, display_name = "cargo-oxichrome")]
struct OxichromeArgs {
    #[command(subcommand)]
    command: OxichromeCommand,
}

#[derive(Subcommand)]
enum OxichromeCommand {
    #[command(about = "Build the extension into dist/chromium or dist/firefox")]
    Build {
        #[arg(long, help = "Optimize the build and shrink the wasm")]
        release: bool,
        #[arg(long, default_value = "chromium", help = "The browser to build for: chromium or firefox")]
        target: String,
    },
    #[command(about = "Remove the dist/ folder")]
    Clean,
    #[command(about = "Print the version of cargo-oxichrome")]
    Version,
    #[command(about = "Make a new extension project")]
    New {
        #[arg(default_value = "my-extension", help = "Folder to create; its last part becomes the crate name")]
        path: String,
        #[arg(long, help = "Only a background service worker in src/lib.rs, no UI and no Leptos", conflicts_with_all = ["side_panel", "no_popup", "no_options"])]
        minimal: bool,
        #[arg(long, help = "Add a side panel (the sidebar in Firefox)")]
        side_panel: bool,
        #[arg(long, help = "Add a content script that runs on example.com")]
        content_script: bool,
        #[arg(long, help = "Put the text in static/_locales/en and read it with t!()")]
        i18n: bool,
        #[arg(long, help = "Leave out the popup")]
        no_popup: bool,
        #[arg(long, help = "Leave out the options page")]
        no_options: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        CargoSubcommand::Oxichrome(args) => match args.command {
            OxichromeCommand::Build { release, target } => {
                let browser = match target.as_str() {
                    "chromium" | "chrome" => Browser::Chromium,
                    "firefox" => Browser::Firefox,
                    other => anyhow::bail!("unknown target browser: {other} (expected \"chromium\" or \"firefox\")"),
                };
                commands::build::run(release, browser)?;
            }
            OxichromeCommand::Clean => commands::clean::run()?,
            OxichromeCommand::Version => println!("cargo-oxichrome {}", env!("CARGO_PKG_VERSION")),
            OxichromeCommand::New { path, minimal, side_panel, content_script, i18n, no_popup, no_options } => {
                let base = if minimal { Parts::minimal() } else { Parts::default() };
                let parts = Parts { popup: base.popup && !no_popup, options: base.options && !no_options, side_panel, content_script, i18n, ..base };
                commands::new::run(&path, &parts)?
            }
        },
    }

    Ok(())
}
