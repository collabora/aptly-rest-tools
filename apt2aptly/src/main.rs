use std::path::PathBuf;

use aptly_rest::{api::snapshots::DeleteOptions, AptlyRest, AptlyRestError};
use clap::Parser;
use color_eyre::{eyre::bail, Result};
use http::StatusCode;
use sync2aptly::{AptlyContent, UploadOptions};
use tracing::{info, metadata::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

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
    /// Root directory of apt repository
    apt_root: PathBuf,
    /// Apt repository distribution
    dist: String,
    /// Import the given apt snapshot and create a new one with the same name
    #[clap(long)]
    snapshot: Option<String>,
    /// If a snapshot already exists with the name, delete it.
    #[clap(long)]
    delete_existing_snapshot: bool,
    /// Maximum number of parallel uploads
    #[clap(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    max_parallel_uploads: u8,
    /// Only show changes, don't apply them
    #[clap(short = 'n', long, default_value_t = false)]
    dry_run: bool,
}

fn is_error_not_found(e: &AptlyRestError) -> bool {
    if let AptlyRestError::Request(e) = e {
        if e.status() == Some(StatusCode::NOT_FOUND) {
            return true;
        }
    }

    false
}

async fn snapshot_exists(aptly: &AptlyRest, snapshot: &str) -> Result<bool> {
    match aptly.snapshot(snapshot).get().await {
        Ok(_) => Ok(true),
        Err(e) if is_error_not_found(&e) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

async fn snapshot_delete(aptly: &AptlyRest, snapshot: &str) -> Result<bool> {
    match aptly
        .snapshot(snapshot)
        .delete(&DeleteOptions { force: true })
        .await
    {
        Ok(_) => Ok(true),
        Err(e) if is_error_not_found(&e) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();
    color_eyre::install().unwrap();
    let opts = Opts::parse();
    let aptly = if let Some(token) = opts.api_token {
        AptlyRest::new_with_token(opts.api_url, &token)?
    } else {
        AptlyRest::new(opts.api_url)
    };

    let dist = if let Some(snapshot) = &opts.snapshot {
        if opts.delete_existing_snapshot {
            if opts.dry_run {
                if snapshot_exists(&aptly, snapshot).await? {
                    info!("Would delete previous snapshot {snapshot}");
                }
            } else if snapshot_delete(&aptly, snapshot).await? {
                info!("Deleted previous snapshot {snapshot}");
            }
        } else if snapshot_exists(&aptly, snapshot).await? {
            bail!("Snapshot {snapshot} already exists");
        }

        format!("{}/snapshots/{}", opts.dist, snapshot)
    } else {
        opts.dist.clone()
    };

    let aptly_contents = AptlyContent::new_from_aptly(&aptly, opts.aptly_repo.clone()).await?;
    let actions = apt2aptly::sync(&opts.apt_root, &dist, aptly.clone(), aptly_contents).await?;
    if !opts.dry_run {
        actions
            .apply(
                "apt2aptly",
                &UploadOptions {
                    max_parallel: opts.max_parallel_uploads,
                },
            )
            .await?;

        if let Some(snapshot) = &opts.snapshot {
            aptly
                .repo(&opts.aptly_repo)
                .snapshot(snapshot, &Default::default())
                .await?;
        }
    }

    Ok(())
}
