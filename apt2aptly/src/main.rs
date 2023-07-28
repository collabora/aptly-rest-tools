use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use aptly_rest::{
    api::{publish, snapshots::DeleteOptions},
    AptlyRest, AptlyRestError,
};
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
    /// Publish the repo and snapshots to the given prefix.
    #[clap(long = "publish-to")]
    publish_prefix: Option<String>,
    /// Use the given GPG key when publishing.
    #[clap(long, requires_if(ArgPredicate::IsPresent, "publish_prefix"))]
    publish_gpg_key: Option<String>,
    /// Import the apt snapshots in the given file and created corresponding
    /// ones in aptly following --aptly-snapshot-template
    #[clap(long, requires_if(ArgPredicate::IsPresent, "aptly_snapshot_template"))]
    apt_snapshots: Option<PathBuf>,
    /// Template to use for creating aptly snapshots (use {component} and
    /// {apt-snapshot} to access the current component and apt snapshot name,
    /// respectively)
    #[clap(long, requires_if(ArgPredicate::IsPresent, "apt_snapshots"))]
    aptly_snapshot_template: Option<String>,
    /// If a snapshot already exists with the name, delete it.
    #[clap(long)]
    delete_existing_snapshot: bool,
    /// If the snapshot is already published, delete it.
    #[clap(long)]
    delete_existing_snapshot_publish: bool,
    /// Maximum number of parallel uploads
    #[clap(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    max_parallel: u8,
    /// Only show changes, don't apply them
    #[clap(short = 'n', long, default_value_t = false)]
    dry_run: bool,
}

const TEMPLATE_VAR_COMPONENT: &str = "component";
const TEMPLATE_VAR_APT_SNAPSHOT: &str = "apt-snapshot";

fn check_template_keys(t: &Template<'_>, expected_keys: &[&str]) -> Result<()> {
    let template_keys = t.keys().collect::<HashSet<_>>();
    let expected_keys = expected_keys.iter().collect::<HashSet<_>>();

    if let Some(key) = template_keys.difference(&expected_keys).next() {
        bail!("Template is using unknown key '{{{key}}}'");
    }
    if let Some(key) = expected_keys.difference(&template_keys).next() {
        bail!("Template is missing key '{{{key}}}'");
    }

    Ok(())
}

fn parse_component_template(s: &str) -> Result<Template<'_>> {
    let t = Template::parse(s)?;
    check_template_keys(&t, &[TEMPLATE_VAR_COMPONENT])?;
    Ok(t)
}

fn parse_snapshot_template(s: &str) -> Result<Template<'_>> {
    let t = Template::parse(s)?;
    check_template_keys(&t, &[TEMPLATE_VAR_COMPONENT, TEMPLATE_VAR_APT_SNAPSHOT])?;
    Ok(t)
}

fn is_error_not_found(e: &AptlyRestError) -> bool {
    if let AptlyRestError::Request(e) = e {
        if e.status() == Some(StatusCode::NOT_FOUND) {
            return true;
        }
    }

    false
}

fn parse_snapshots_list(path: &Path) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let mut lines = vec![];
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        lines.push(line);
    }

    Ok(lines)
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

enum AptDist<'s, 't> {
    Dist(&'s str),
    Snapshot {
        dist: &'s str,
        snapshot: &'s str,
        template: &'s Template<'t>,
    },
}

impl AptDist<'_, '_> {
    fn path(&self) -> String {
        match self {
            AptDist::Dist(dist) => (*dist).to_owned(),
            AptDist::Snapshot { dist, snapshot, .. } => format!("{dist}/snapshots/{snapshot}"),
        }
    }
}

struct AptRepo<'s, 't> {
    root: &'s Url,
    dist: AptDist<'s, 't>,
}

async fn sync_dist(
    aptly: &AptlyRest,
    aptly_repo_template: &Template<'_>,
    apt_repo: &AptRepo<'_, '_>,
    opts: &Opts,
) -> Result<()> {
    let mut sources = vec![];

    let dist_path = apt_repo.dist.path();
    let scanner = apt2aptly::DistScanner::new(apt_repo.root, &dist_path).await?;

    for component in scanner.components() {
        let aptly_repo = aptly_repo_template
            .render(&HashMap::from([(TEMPLATE_VAR_COMPONENT, &component)]))
            .wrap_err("Failed to render aptly repo template")?;
        let aptly_snapshot = if let AptDist::Snapshot {
            snapshot, template, ..
        } = &apt_repo.dist
        {
            Some(template.render(&HashMap::from([
                (TEMPLATE_VAR_COMPONENT, component.as_str()),
                (TEMPLATE_VAR_APT_SNAPSHOT, snapshot),
            ]))?)
        } else {
            None
        };

        if let Some(aptly_snapshot) = &aptly_snapshot {
            sources.push(publish::Source {
                name: aptly_snapshot.clone(),
                component: Some(component.clone()),
            });

            if opts.delete_existing_snapshot {
                if opts.dry_run {
                    if snapshot_exists(aptly, aptly_snapshot).await? {
                        info!("Would delete previous snapshot {aptly_snapshot}");
                        continue;
                    }
                } else if snapshot_delete(aptly, aptly_snapshot).await? {
                    info!("Deleted previous snapshot {aptly_snapshot}");
                }
            } else if snapshot_exists(aptly, aptly_snapshot).await? {
                warn!("Snapshot {aptly_snapshot} already exists, skipping...");
                continue;
            }
        } else {
            sources.push(publish::Source {
                name: aptly_repo.clone(),
                component: Some(component.clone()),
            });
        }

        let aptly_contents = AptlyContent::new_from_aptly(aptly, aptly_repo.clone()).await?;
        let actions = scanner
            .sync_component(component, aptly.clone(), aptly_contents)
            .await?;
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
                aptly
                    .repo(&aptly_repo)
                    .snapshot(&aptly_snapshot, &Default::default())
                    .await?;
            }
        }
    }

    if let Some(publish_prefix) = &opts.publish_prefix {
        let published = aptly.published().await?;
        let existing_publish = published
            .into_iter()
            .any(|p| p.prefix() == publish_prefix && p.distribution() == dist_path);

        if existing_publish {
            if matches!(apt_repo.dist, AptDist::Snapshot { .. })
                && opts.delete_existing_snapshot_publish
            {
                if opts.dry_run {
                    info!(
                        "Would delete previous published distribution {}/{}",
                        publish_prefix, dist_path
                    );
                } else {
                    aptly
                        .publish_prefix(publish_prefix)
                        .distribution(&dist_path)
                        .delete(&publish::DeleteOptions { force: true })
                        .await?;
                    info!(
                        "Deleted previous published distribution {}/{}",
                        publish_prefix, dist_path
                    );
                }
            } else {
                warn!("Publish prefix {} already exists, skipping", publish_prefix);
                return Ok(());
            }
        }

        if opts.dry_run {
            info!("Would publish to {}/{}", publish_prefix, dist_path);
        } else {
            let kind = match &apt_repo.dist {
                AptDist::Dist(_) => publish::SourceKind::Local,
                AptDist::Snapshot { .. } => publish::SourceKind::Snapshot,
            };

            let signing = if let Some(key) = &opts.publish_gpg_key {
                publish::Signing::Enabled(publish::SigningOptions {
                    gpg_key: Some(key.clone()),
                    ..Default::default()
                })
            } else {
                publish::Signing::Disabled
            };

            info!("Publishing to {}/{}...", publish_prefix, dist_path);
            aptly
                .publish_prefix(publish_prefix)
                .publish(
                    kind,
                    &sources,
                    &publish::PublishOptions {
                        distribution: Some(dist_path),
                        signing: Some(signing),
                        ..Default::default()
                    },
                )
                .await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();
    color_eyre::install().unwrap();
    let opts = Opts::parse();
    let aptly = if let Some(token) = &opts.api_token {
        AptlyRest::new_with_token(opts.api_url.clone(), token)?
    } else {
        AptlyRest::new(opts.api_url.clone())
    };

    let aptly_repo_template = parse_component_template(&opts.aptly_repo_template)
        .wrap_err("Failed to parse aptly repo template")?;
    let aptly_snapshot_template = opts
        .aptly_snapshot_template
        .as_deref()
        .map(parse_snapshot_template)
        .transpose()
        .wrap_err("Failed to parse aptly snapshot template")?;

    if let Some(snapshots_path) = &opts.apt_snapshots {
        for snapshot in parse_snapshots_list(snapshots_path)? {
            sync_dist(
                &aptly,
                &aptly_repo_template,
                &AptRepo {
                    root: &opts.apt_root,
                    dist: AptDist::Snapshot {
                        dist: &opts.dist,
                        snapshot: &snapshot,
                        template: aptly_snapshot_template.as_ref().unwrap(),
                    },
                },
                &opts,
            )
            .await?;
        }
    }

    sync_dist(
        &aptly,
        &aptly_repo_template,
        &AptRepo {
            root: &opts.apt_root,
            dist: AptDist::Dist(&opts.dist),
        },
        &opts,
    )
    .await?;

    Ok(())
}
