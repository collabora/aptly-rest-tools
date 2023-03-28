use std::{io::stdout, process::ExitCode};

use aptly_rest::{api::repos, AptlyRest, AptlyRestError};
use clap::{Parser, Subcommand};
use color_eyre::Result;
use http::StatusCode;
use tracing::{debug, info};

use crate::OutputFormat;

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
    TestExists(RepoTestExistsOpts),
    Search(RepoSearchOpts),
    Snapshot(RepoSnapshotOpts),
    Clean(RepoCleanOpts),
    Drop(RepoDropOpts),
}

impl RepoCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            RepoCommand::Create(args) => {
                let repo = aptly
                    .create_repo(
                        &repos::Repo::new(args.repo.clone())
                            .with_component(args.component.clone())
                            .with_distribution(args.distribution.clone()),
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

            RepoCommand::TestExists(args) => {
                if let Err(err) = aptly.repo(&args.repo).get().await {
                    let AptlyRestError::Request(err) = err;
                    if err.status() == Some(StatusCode::NOT_FOUND) {
                        return Ok(ExitCode::FAILURE);
                    } else {
                        return Err(err.into());
                    }
                }
            }

            RepoCommand::Search(args) => match args.format {
                OutputFormat::Name => {
                    let mut keys = aptly
                        .repo(&args.repo)
                        .packages()
                        .query(args.query.clone(), false)
                        .list()
                        .await?;
                    if args.exit_code && keys.is_empty() {
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
                        .query(args.query.clone(), false)
                        .detailed()
                        .await?;
                    if args.exit_code && results.is_empty() {
                        return Ok(ExitCode::FAILURE);
                    }

                    serde_json::to_writer_pretty(&mut stdout(), &results)?;
                }
            },

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
