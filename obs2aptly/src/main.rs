use std::path::PathBuf;

use aptly_rest::AptlyRest;
use clap::Parser;
use color_eyre::Result;
use obs2aptly::{AptlyContent, ObsContent};
use tracing::metadata::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

#[derive(Parser, Debug)]
struct Opts {
    /// Url for the aptly rest api endpoint
    #[clap(short, long, default_value = "http://localhost:8080")]
    api_url: url::Url,
    /// Authentication token for the API
    #[clap(long, env = "APTLY_API_TOKEN")]
    api_token: Option<String>,
    /// Repo in aptly
    aptly_repo: String,
    /// Directory with obs repositories
    obs_repo: PathBuf,
    /// Only show changes, don't apply them
    #[clap(short = 'n', long, default_value_t = false)]
    dry_run: bool,
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

    let aptly_contents = AptlyContent::new_from_aptly(&aptly, opts.aptly_repo).await?;
    let obs_content = ObsContent::new_from_path(opts.obs_repo).await?;

    let actions = obs2aptly::sync(aptly, obs_content, aptly_contents).await?;
    if !opts.dry_run {
        actions.apply().await?;
    }

    Ok(())
}
