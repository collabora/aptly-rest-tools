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

#[derive(Parser, Debug)]
pub struct MirrorCreateOpts {
    name: String,
    url: Url,
    #[clap(long)]
    distribution: Option<String>,
    #[clap(long)]
    filter: Option<String>,
    #[clap(long = "component")]
    components: Vec<String>,
    #[clap(long = "architecture")]
    architectures: Vec<String>,
    #[clap(long = "keyring")]
    keyrings: Vec<String>,
    #[clap(long)]
    download_sources: bool,
    #[clap(long)]
    download_udebs: bool,
    #[clap(long)]
    download_installer: bool,
    #[clap(long)]
    download_app_stream: bool,
    #[clap(long)]
    filter_with_deps: bool,
    #[clap(long)]
    skip_component_check: bool,
    #[clap(long)]
    skip_architecture_check: bool,
    #[clap(long)]
    ignore_signatures: bool,
}

async fn create_mirror(opts: &MirrorCreateOpts, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(&opts.name);
    let mut creation = mirror.create(opts.url.clone());

    if let Some(distribution) = &opts.distribution {
        creation.distribution(distribution);
    }
    if let Some(filter) = &opts.filter {
        creation.filter(filter);
    }
    if !opts.components.is_empty() {
        creation.components(opts.components.clone());
    }
    if !opts.architectures.is_empty() {
        creation.architectures(opts.architectures.clone());
    }
    if !opts.keyrings.is_empty() {
        creation.keyrings(opts.keyrings.clone());
    }
    if opts.download_sources {
        creation.download_sources(true);
    }
    if opts.download_udebs {
        creation.download_udebs(true);
    }
    if opts.download_installer {
        creation.download_installer(true);
    }
    if opts.download_app_stream {
        creation.download_app_stream(true);
    }
    if opts.filter_with_deps {
        creation.filter_with_deps(true);
    }
    if opts.skip_component_check {
        creation.skip_component_check(true);
    }
    if opts.skip_architecture_check {
        creation.skip_architecture_check(true);
    }
    if opts.ignore_signatures {
        creation.ignore_signatures(true);
    }

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
// MirrorCreateOpts is larger than the other variants, but this enum is only ever
// constructed once from the parsed command line, so the size difference is irrelevant
// (and clap's derive does not support boxing the variant).
#[allow(clippy::large_enum_variant)]
pub enum MirrorCommand {
    List(MirrorListOpts),
    Create(MirrorCreateOpts),
    Update { name: String },
    Drop { name: String },
}

impl MirrorCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            MirrorCommand::List(args) => list(args.format, aptly).await?,
            MirrorCommand::Create(args) => create_mirror(args, aptly).await?,
            MirrorCommand::Update { name } => update(name, aptly).await?,
            MirrorCommand::Drop { name } => drop_mirror(name, aptly).await?,
        }
        Ok(ExitCode::SUCCESS)
    }
}
