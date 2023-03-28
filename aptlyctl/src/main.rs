use std::process::ExitCode;

use aptly_rest::AptlyRest;
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use publish::PublishCommand;
use repo::RepoCommand;
use snapshot::SnapshotCommand;
use tracing::{info, metadata::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

mod publish;
mod repo;
mod snapshot;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum OutputFormat {
    Name,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Name
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    Repo {
        #[clap(subcommand)]
        command: RepoCommand,
    },
    Publish {
        #[clap(subcommand)]
        command: PublishCommand,
    },
    Snapshot {
        #[clap(subcommand)]
        command: SnapshotCommand,
    },
    DbCleanup,
}

#[derive(Parser, Debug)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
    /// Url for the aptly rest api endpoint
    #[clap(short, long, default_value = "http://localhost:8080")]
    url: url::Url,
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();
    color_eyre::install().unwrap();
    let opts = Opts::parse();
    let aptly = AptlyRest::new(opts.url);

    match opts.command {
        Command::Repo { command } => command.run(&aptly).await,
        Command::Publish { command } => command.run(&aptly).await,
        Command::Snapshot { command } => command.run(&aptly).await,
        Command::DbCleanup => {
            aptly.db_cleanup().await?;
            info!("Ran database cleanup");
            Ok(ExitCode::SUCCESS)
        }
    }
}
