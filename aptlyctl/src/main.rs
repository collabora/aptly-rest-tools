use std::process::ExitCode;

use aptly_rest::AptlyRest;
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use mirror::MirrorCommand;
use publish::PublishCommand;
use repo::RepoCommand;
use snapshot::SnapshotCommand;
use tools::ToolsCommand;
use tracing::{info, metadata::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

mod mirror;
mod publish;
mod repo;
mod snapshot;
mod tools;

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
enum OutputFormat {
    #[default]
    Name,
    Json,
    Yaml,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
enum LogFormat {
    #[default]
    Pretty,
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
    Mirror {
        #[clap(subcommand)]
        command: MirrorCommand,
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
    /// Log output format
    #[clap(long, value_enum, default_value_t = LogFormat::Pretty)]
    log_format: LogFormat,
}

fn init_tracing(format: LogFormat) {
    match format {
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(ErrorLayer::default())
            .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
            .init(),
        LogFormat::Json => tracing_subscriber::registry()
            .with(ErrorLayer::default())
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_filter(LevelFilter::INFO),
            )
            .init(),
    }
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let opts = Opts::parse();
    init_tracing(opts.log_format);
    color_eyre::install()?;

    let aptly = if let Some(token) = opts.api_token {
        AptlyRest::new_with_token(opts.api_url, &token)?
    } else {
        AptlyRest::new(opts.api_url)
    };

    match opts.command {
        Command::Repo { command } => command.run(&aptly).await,
        Command::Publish { command } => command.run(&aptly).await,
        Command::Snapshot { command } => command.run(&aptly).await,
        Command::Tools { command } => command.run().await,
        Command::Mirror { command } => command.run(&aptly).await,
        Command::DbCleanup => {
            aptly.db_cleanup().await?;
            info!("Ran database cleanup");
            Ok(ExitCode::SUCCESS)
        }
    }
}
