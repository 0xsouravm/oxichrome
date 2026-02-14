use clap::{Parser, Subcommand};
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
        #[arg(default_value = "my-extension")]
        name: String,
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
            OxichromeCommand::New { name } => commands::new::run(&name)?,
        },
    }

    Ok(())
}
