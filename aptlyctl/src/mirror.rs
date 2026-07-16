use std::{io::stdout, process::ExitCode};

use aptly_rest::AptlyRest;
use clap::{Parser, Subcommand};
use color_eyre::Result;
use url::Url;

use crate::OutputFormat;

#[derive(Parser, Debug)]
pub struct MirrorListOpts {
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

async fn create_mirror(name: &str, url: &Url, dist: &str, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(name);
    let mut creation = mirror.create(url.clone());
    creation.distribution(dist).ignore_signatures(true);
    creation.run().await?;

    Ok(())
}

async fn update(name: &str, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(name);
    mirror.update().run().await?;
    Ok(())
}

async fn drop_mirror(name: &str, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(name);
    mirror.drop().await?;
    Ok(())
}

async fn list(format: OutputFormat, aptly: &AptlyRest) -> Result<()> {
    let mirrors = aptly.mirrors().await?;
    match format {
        OutputFormat::Name => {
            let mut names: Vec<_> = mirrors.iter().map(|m| m.name()).collect();
            names.sort();
            for name in names {
                println!("{name}");
            }
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut stdout(), &mirrors)?;
            println!();
        }
        OutputFormat::Yaml => {
            serde_saphyr::to_io_writer(&mut stdout(), &mirrors)?;
            println!();
        }
    }
    Ok(())
}

#[derive(Subcommand, Debug)]
pub enum MirrorCommand {
    List(MirrorListOpts),
    Create {
        name: String,
        url: Url,
        dist: String,
    },
    Update {
        name: String,
    },
    Drop {
        name: String,
    },
}

impl MirrorCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            MirrorCommand::List(args) => list(args.format, aptly).await?,
            MirrorCommand::Create { name, url, dist } => {
                create_mirror(name, url, dist, aptly).await?
            }
            MirrorCommand::Update { name } => update(name, aptly).await?,
            MirrorCommand::Drop { name } => drop_mirror(name, aptly).await?,
        }
        Ok(ExitCode::SUCCESS)
    }
}
