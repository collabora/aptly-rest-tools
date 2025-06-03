use std::{path::PathBuf, process::ExitCode};

use aptly_rest::{dsc::Dsc, key::AptlyKey};
use clap::{Parser, Subcommand};
use color_eyre::Result;

#[derive(Parser, Debug)]
pub struct ToolsComputeKeyOpts {
    dsc: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum ToolsCommand {
    ComputeKey(ToolsComputeKeyOpts),
}

impl ToolsCommand {
    pub async fn run(self) -> Result<ExitCode> {
        match self {
            ToolsCommand::ComputeKey(args) => {
                let dsc = Dsc::from_file(args.dsc).await?;
                let key = AptlyKey::try_from(&dsc)?;
                println!("{key}");
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
