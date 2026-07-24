use std::path::PathBuf;

use aptly_rest::AptlyRest;
use clap::{Parser, ValueEnum};
use color_eyre::Result;
use sync2aptly::{AptlyContent, PoolPackagesCache, UploadOptions};
use tracing::metadata::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
enum FilterKind {
    Sources,
    Binaries,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
enum LogFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Parser, Debug)]
struct Opts {
    /// Url for the aptly rest api endpoint
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
    /// Repo in aptly
    aptly_repo: String,
    /// Directory with obs repositories
    obs_repo: PathBuf,
    /// Maximum number of parallel uploads
    #[clap(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    max_parallel_uploads: u8,
    /// Only sync files of the given type
    #[clap(long)]
    only: Option<FilterKind>,
    /// Only show changes, don't apply them
    #[clap(short = 'n', long, default_value_t = false)]
    dry_run: bool,
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
async fn main() -> Result<()> {
    let opts = Opts::parse();
    init_tracing(opts.log_format);
    color_eyre::install()?;

    let aptly = if let Some(token) = opts.api_token {
        AptlyRest::new_with_token(opts.api_url, &token)?
    } else {
        AptlyRest::new(opts.api_url)
    };

    let aptly_contents = AptlyContent::new_from_aptly(&aptly, opts.aptly_repo).await?;
    let pool_packages = PoolPackagesCache::new(aptly.clone());
    let actions = obs2aptly::sync(
        opts.obs_repo,
        aptly,
        aptly_contents,
        pool_packages,
        &obs2aptly::ScanOptions {
            include_binaries: opts
                .only
                .as_ref()
                .is_none_or(|only| *only == FilterKind::Binaries),
            include_sources: opts
                .only
                .as_ref()
                .is_none_or(|only| *only == FilterKind::Sources),
        },
    )
    .await?;
    if !opts.dry_run {
        actions
            .apply(
                "obs2aptly",
                &UploadOptions {
                    max_parallel: opts.max_parallel_uploads,
                },
            )
            .await?;
    }

    Ok(())
}
