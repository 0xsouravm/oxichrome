use clap::{Parser, Subcommand};
use oxichrome_build::templates::Parts;
use oxichrome_build::Browser;

mod commands;

#[derive(Parser)]
#[command(name = "cargo-oxichrome", bin_name = "cargo")]
struct Cli {
    #[command(subcommand)]
    command: CargoSubcommand,
}

#[derive(Subcommand)]
enum CargoSubcommand {
    Oxichrome(OxichromeArgs),
}

#[derive(Parser)]
struct OxichromeArgs {
    #[command(subcommand)]
    command: OxichromeCommand,
}

#[derive(Subcommand)]
enum OxichromeCommand {
    Build {
        #[arg(long)]
        release: bool,
        #[arg(long, default_value = "chromium")]
        target: String,
    },
    Clean,
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
            OxichromeCommand::New { path, minimal, side_panel, content_script, i18n, no_popup, no_options } => {
                let base = if minimal { Parts::minimal() } else { Parts::default() };
                let parts = Parts { popup: base.popup && !no_popup, options: base.options && !no_options, side_panel, content_script, i18n, ..base };
                commands::new::run(&path, &parts)?
            }
        },
    }

    Ok(())
}
