use std::{io::stdout, process::ExitCode};

use aptly_rest::{AptlyRest, AptlyRestError, TaskOr};
use clap::{Parser, Subcommand};
use color_eyre::Result;
use http::StatusCode;
use tracing::info;

use crate::OutputFormat;

#[derive(Parser, Debug)]
pub struct SnapshotListOpts {
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Parser, Debug)]
pub struct SnapshotTestExistsOpts {
    snapshot: String,
}

#[derive(Parser, Debug)]
pub struct SnapshotDropOpts {
    snapshot: String,
    #[clap(long)]
    force: bool,
}

#[derive(Subcommand, Debug)]
pub enum SnapshotCommand {
    List(SnapshotListOpts),
    TestExists(SnapshotTestExistsOpts),
    Drop(SnapshotDropOpts),
}

impl SnapshotCommand {
    pub async fn run(self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            SnapshotCommand::List(args) => {
                let snapshots = aptly.snapshots().await?;
                match args.format {
                    OutputFormat::Name => {
                        let mut names: Vec<_> = snapshots.iter().map(|s| s.name()).collect();
                        names.sort();
                        for name in names {
                            println!("{}", name);
                        }
                    }
                    OutputFormat::Json => {
                        serde_json::to_writer_pretty(&mut stdout(), &snapshots)?;
                        println!();
                    }
                }
            }

            SnapshotCommand::TestExists(args) => match aptly.snapshot(&args.snapshot).get().await {
                Ok(_) => {}
                Err(AptlyRestError::Request(ref e))
                    if e.status() == Some(StatusCode::NOT_FOUND) =>
                {
                    return Ok(ExitCode::FAILURE);
                }
                Err(err) => return Err(err.into()),
            },

            SnapshotCommand::Drop(args) => {
                let result = aptly
                    .snapshot(&args.snapshot)
                    .delete(&aptly_rest::api::snapshots::DeleteOptions { force: args.force })
                    .await?;
                match result {
                    TaskOr::Value(_) => {
                        info!("Deleted snapshot '{}'", args.snapshot);
                    }
                    TaskOr::Task(task) => {
                        info!("{task}");
                    }
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
