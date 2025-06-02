use std::{io::stdout, process::ExitCode};

use aptly_rest::{api::repos, key::AptlyKey, AptlyRest, AptlyRestError};
use clap::{Parser, Subcommand};
use color_eyre::Result;
use http::StatusCode;
use tracing::{debug, info, warn};

use crate::OutputFormat;

#[derive(Parser, Debug, Clone)]
pub struct RepoPackagesListOpts {
    repo: String,
    #[clap(long, short, default_value("Name"))]
    query: String,
    #[clap(long, short)]
    fail_if_empty: bool,
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Parser, Debug, Clone)]
pub struct RepoPackagesDeleteOpts {
    repo: String,
    #[clap(long = "key", short, required_unless_present("queries"))]
    keys: Vec<AptlyKey>,
    #[clap(long = "query", short, required_unless_present("keys"))]
    queries: Vec<String>,
    #[clap(long, short = 'n', default_value_t)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum RepoPackagesCommand {
    List(RepoPackagesListOpts),
    Delete(RepoPackagesDeleteOpts),
}

impl RepoPackagesCommand {
    pub async fn run(self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            RepoPackagesCommand::List(args) => match args.format {
                OutputFormat::Name => {
                    let mut keys = aptly
                        .repo(&args.repo)
                        .packages()
                        .query(args.query, false)
                        .list()
                        .await?;
                    if args.fail_if_empty && keys.is_empty() {
                        return Ok(ExitCode::FAILURE);
                    }

                    keys.sort();
                    for key in keys {
                        println!("{}", key);
                    }
                }
                OutputFormat::Json => {
                    let results = aptly
                        .repo(&args.repo)
                        .packages()
                        .query(args.query, false)
                        .detailed()
                        .await?;
                    if args.fail_if_empty && results.is_empty() {
                        return Ok(ExitCode::FAILURE);
                    }

                    serde_json::to_writer_pretty(&mut stdout(), &results)?;
                }
            },
            RepoPackagesCommand::Delete(mut args) => {
                for query in args.queries {
                    info!("Finding packages for query '{query}'...");
                    let keys = aptly
                        .repo(&args.repo)
                        .packages()
                        .query(query, false)
                        .list()
                        .await?;
                    info!("Query found {} package(s)", keys.len());
                    for key in &keys {
                        info!("{key}");
                    }
                    args.keys.extend(keys.into_iter());
                }

                if args.keys.is_empty() {
                    info!("No packages to delete");
                    return Ok(ExitCode::SUCCESS);
                }

                if args.dry_run {
                    info!("Would delete {} package(s)", args.keys.len());
                } else {
                    info!("Deleting {} package(s)...", args.keys.len());

                    aptly
                        .repo(&args.repo)
                        .packages()
                        .delete(args.keys.iter())
                        .await?;
                    info!("Deletion complete");
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}

#[derive(Parser, Debug)]
pub struct RepoCreateOpts {
    repo: String,
    #[clap(long)]
    component: Option<String>,
    #[clap(long)]
    distribution: Option<String>,
}

#[derive(Parser, Debug)]
pub struct RepoListOpts {
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Parser, Debug)]
pub struct RepoTestExistsOpts {
    repo: String,
}

#[derive(Parser, Debug)]
pub struct RepoSearchOpts {
    repo: String,
    query: String,
    #[clap(long, short)]
    exit_code: bool,
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Parser, Debug)]
pub struct RepoSnapshotOpts {
    repo: String,
    snapshot: String,
}

#[derive(Parser, Debug)]
pub struct RepoCleanOpts {
    repo: String,
}

#[derive(Parser, Debug)]
pub struct RepoDropOpts {
    repo: String,
    #[clap(long)]
    force: bool,
}

#[derive(Subcommand, Debug)]
pub enum RepoCommand {
    Create(RepoCreateOpts),
    List(RepoListOpts),
    #[clap(subcommand)]
    Packages(RepoPackagesCommand),
    TestExists(RepoTestExistsOpts),
    #[clap(hide(true))]
    Search(RepoSearchOpts),
    Snapshot(RepoSnapshotOpts),
    Clean(RepoCleanOpts),
    Drop(RepoDropOpts),
}

impl RepoCommand {
    pub async fn run(self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            RepoCommand::Create(args) => {
                let repo = aptly
                    .create_repo(
                        &repos::Repo::new(args.repo)
                            .with_component(args.component)
                            .with_distribution(args.distribution),
                    )
                    .await?;
                debug!(?repo);
                info!("Created repo '{}'", repo.name());
            }

            RepoCommand::List(args) => {
                let repos = aptly.repos().await?;
                match args.format {
                    OutputFormat::Name => {
                        let mut names: Vec<_> = repos.iter().map(|r| r.name()).collect();
                        names.sort();
                        for name in names {
                            println!("{}", name);
                        }
                    }
                    OutputFormat::Json => {
                        serde_json::to_writer_pretty(&mut stdout(), &repos)?;
                        println!();
                    }
                }
            }

            RepoCommand::Packages(command) => return command.run(aptly).await,

            RepoCommand::Search(args) => {
                warn!("'aptlyctl repo search <REPO> <QUERY>' is deprecated");
                warn!("Use 'aptlyctl repo packages list -q <QUERY> <REPO>' instead");
                return RepoPackagesCommand::List(RepoPackagesListOpts {
                    repo: args.repo,
                    query: args.query,
                    fail_if_empty: args.exit_code,
                    format: args.format,
                })
                .run(aptly)
                .await;
            }

            RepoCommand::TestExists(args) => {
                if let Err(err) = aptly.repo(&args.repo).get().await {
                    if let AptlyRestError::Request(err) = &err {
                        if err.status() == Some(StatusCode::NOT_FOUND) {
                            return Ok(ExitCode::FAILURE);
                        }
                    }

                    return Err(err.into());
                }
            }

            RepoCommand::Snapshot(args) => {
                let snapshot = aptly
                    .repo(&args.repo)
                    .snapshot(&args.snapshot, &Default::default())
                    .await?;
                info!(
                    "Created snapshot '{}' of repo '{}'",
                    snapshot.name(),
                    args.repo
                );
            }

            RepoCommand::Clean(args) => {
                info!("Finding packages to delete...");
                let packages = aptly.repo(&args.repo).packages().list().await?;
                info!("Deleting {} package(s)...", packages.len());
                aptly
                    .repo(&args.repo)
                    .packages()
                    .delete(packages.iter())
                    .await?;
                info!("Deletion complete");
            }

            RepoCommand::Drop(args) => {
                aptly
                    .repo(&args.repo)
                    .delete(&repos::DeleteOptions { force: args.force })
                    .await?;
                info!("Deleted repo '{}'", args.repo);
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
