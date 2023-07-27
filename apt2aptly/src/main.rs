use std::collections::HashMap;

use aptly_rest::{api::snapshots::DeleteOptions, AptlyRest, AptlyRestError};
use clap::{builder::ArgPredicate, Parser};
use color_eyre::{
    eyre::{bail, Context},
    Result,
};
use http::StatusCode;
use leon::Template;
use sync2aptly::{AptlyContent, UploadOptions};
use tracing::{info, metadata::LevelFilter, warn};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use url::Url;

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
    /// Template to use as the aptly repo (use {component} to access the current
    /// component)
    aptly_repo_template: String,
    /// Root URL of apt repository
    apt_root: Url,
    /// Apt repository distribution
    dist: String,
    /// Import the given apt snapshot and create one in aptly following
    /// --aptly-snapshot-template
    #[clap(long, requires_if(ArgPredicate::IsPresent, "aptly_snapshot_template"))]
    apt_snapshot: Option<String>,
    /// Template to use for creating aptly snapshots (use {component} and
    /// {apt-snapshot} to access the current component and apt snapshot name,
    /// respectively)
    #[clap(long, requires_if(ArgPredicate::IsPresent, "apt_snapshot"))]
    aptly_snapshot_template: Option<String>,
    /// If a snapshot already exists with the name, delete it.
    #[clap(long)]
    delete_existing_snapshot: bool,
    /// Maximum number of parallel uploads
    #[clap(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    max_parallel: u8,
    /// Only show changes, don't apply them
    #[clap(short = 'n', long, default_value_t = false)]
    dry_run: bool,
}

fn parse_component_template(s: &str) -> Result<Template<'_>> {
    match Template::parse(s) {
        Ok(t) if t.has_key("component") => Ok(t),
        Ok(_) => bail!("Template is missing '{{component}}'"),
        Err(e) => Err(e.into()),
    }
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

    let dist = if let Some(snapshot) = &opts.apt_snapshot {
        format!("{}/snapshots/{}", opts.dist, snapshot)
    } else {
        opts.dist.clone()
    };

    let aptly_repo_template = parse_component_template(&opts.aptly_repo_template)
        .wrap_err("Failed to parse aptly repo template")?;
    let aptly_snapshot_template = opts
        .aptly_snapshot_template
        .as_deref()
        .map(parse_component_template)
        .transpose()
        .wrap_err("Failed to parse aptly snapshot template")?;

    for (component, origin_content) in apt2aptly::scan_dist(&opts.apt_root, &dist).await? {
        let aptly_repo = aptly_repo_template
            .render(&HashMap::from([("component", &component)]))
            .wrap_err("Failed to render aptly repo template")?;
        let aptly_snapshot = aptly_snapshot_template
            .as_ref()
            .map(|t| {
                t.render(&HashMap::from([
                    ("component", &component),
                    ("apt-snapshot", opts.apt_snapshot.as_ref().unwrap()),
                ]))
            })
            .transpose()
            .wrap_err("Failed to render aptly snapshot template")?;
        let aptly_contents = AptlyContent::new_from_aptly(&aptly, aptly_repo.clone()).await?;

        let actions = sync2aptly::sync(origin_content, aptly.clone(), aptly_contents).await?;
        if !opts.dry_run {
            actions
                .apply(
                    "apt2aptly",
                    &UploadOptions {
                        max_parallel: opts.max_parallel,
                    },
                )
                .await?;

            if let Some(aptly_snapshot) = aptly_snapshot {
                if opts.delete_existing_snapshot {
                    if opts.dry_run {
                        if snapshot_exists(&aptly, &aptly_snapshot).await? {
                            info!("Would delete previous snapshot {aptly_snapshot}");
                            continue;
                        }
                    } else if snapshot_delete(&aptly, &aptly_snapshot).await? {
                        info!("Deleted previous snapshot {aptly_snapshot}");
                    }
                } else if snapshot_exists(&aptly, &aptly_snapshot).await? {
                    warn!("Snapshot {aptly_snapshot} already exists, skipping...");
                    continue;
                }

                aptly
                    .repo(&aptly_repo)
                    .snapshot(&aptly_snapshot, &Default::default())
                    .await?;
            }
        }
    }

    Ok(())
}
