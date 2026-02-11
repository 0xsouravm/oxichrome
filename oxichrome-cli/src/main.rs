use clap::{Parser, Subcommand};

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
    },
    New {
        #[arg(default_value = "my-extension")]
        name: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        CargoSubcommand::Oxichrome(args) => match args.command {
            OxichromeCommand::Build { release } => commands::build::run(release)?,
            OxichromeCommand::New { name } => commands::new::run(&name)?,
        },
    }

    Ok(())
}
