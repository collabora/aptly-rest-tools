use std::process::ExitCode;

use aptly_rest::{AptlyRest, TaskOr};
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use publish::PublishCommand;
use repo::RepoCommand;
use snapshot::SnapshotCommand;
use tools::ToolsCommand;
use tracing::{info, metadata::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

mod publish;
mod repo;
mod snapshot;
mod tools;

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
enum OutputFormat {
    #[default]
    Name,
    Json,
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
    Tools {
        #[clap(subcommand)]
        command: ToolsCommand,
    },
    DbCleanup,
}

#[derive(Parser, Debug)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
    /// Url for the aptly rest API endpoint
    #[clap(
        short = 'u',
        long,
        env = "APTLY_API_URL",
        default_value = "http://localhost:8080"
    )]
    api_url: url::Url,
    /// Authentication token for the API
    #[clap(long, env = "APTLY_API_TOKEN")]
    api_token: Option<String>,
    /// Run the operation asynchronously on the server
    #[clap(long = "async")]
    async_mode: bool,
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();
    color_eyre::install().unwrap();
    let opts = Opts::parse();
    let mut aptly = if let Some(token) = opts.api_token {
        AptlyRest::new_with_token(opts.api_url, &token)?
    } else {
        AptlyRest::new(opts.api_url)
    };
    aptly.async_mode = opts.async_mode;

    match opts.command {
        Command::Repo { command } => command.run(&aptly).await,
        Command::Publish { command } => command.run(&aptly).await,
        Command::Snapshot { command } => command.run(&aptly).await,
        Command::Tools { command } => command.run().await,
        Command::DbCleanup => {
            let result = aptly.db_cleanup().await?;
            match result {
                TaskOr::Value(_) => {
                    info!("Ran database cleanup");
                }
                TaskOr::Task(task) => {
                    info!("{task}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
