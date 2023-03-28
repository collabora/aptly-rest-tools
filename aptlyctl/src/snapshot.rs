use std::process::ExitCode;

use aptly_rest::{AptlyRest, AptlyRestError};
use clap::{Parser, Subcommand};
use color_eyre::Result;
use http::StatusCode;
use tracing::info;

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
    TestExists(SnapshotTestExistsOpts),
    Drop(SnapshotDropOpts),
}

impl SnapshotCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            SnapshotCommand::TestExists(args) => {
                if let Err(err) = aptly.snapshot(&args.snapshot).get().await {
                    let AptlyRestError::Request(err) = err;
                    if err.status() == Some(StatusCode::NOT_FOUND) {
                        return Ok(ExitCode::FAILURE);
                    } else {
                        return Err(err.into());
                    }
                }
            }
            SnapshotCommand::Drop(args) => {
                aptly
                    .snapshot(&args.snapshot)
                    .delete(&Default::default())
                    .await?;
                info!("Deleted snapshot '{}'", args.snapshot);
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
