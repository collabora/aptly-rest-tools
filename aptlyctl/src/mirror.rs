use std::process::ExitCode;

use aptly_rest::AptlyRest;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use debian_packaging::repository::{http::HttpRepositoryClient, RepositoryRootReader};
use tracing::info;
use url::Url;

#[derive(Parser, Debug)]
pub struct MirrorCreateOpts {
    url: Url,
    dist: String,
}

async fn create_mirror(url: &Url, dist: &str, aptly: &AptlyRest) -> Result<()> {
    let repo = HttpRepositoryClient::new(url.clone())?;

    let release = repo.release_reader(dist).await?;
    for c in release
        .release_file()
        .components()
        .ok_or_else(|| eyre!("No components found"))?
    {
        info!("Component {c}");
    }

    Ok(())
}

#[derive(Subcommand, Debug)]
pub enum MirrorCommand {
    Create { url: Url, dist: String },
}

impl MirrorCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        info!("mirror");
        match self {
            MirrorCommand::Create { url, dist } => create_mirror(url, &dist, aptly).await?,
        }
        Ok(ExitCode::SUCCESS)
    }
}
