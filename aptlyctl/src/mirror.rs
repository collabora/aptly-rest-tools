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

#[derive(Parser, Debug)]
pub struct MirrorUpdateOpts {
    name: String,
    #[clap(long)]
    rename: Option<String>,
    #[clap(long = "keyring")]
    keyrings: Vec<String>,
    #[clap(long)]
    ignore_checksums: bool,
    #[clap(long)]
    ignore_signatures: bool,
    #[clap(long)]
    force_update: bool,
    #[clap(long)]
    skip_existing_packages: bool,
    #[clap(long)]
    latest_only: bool,
}

async fn update(opts: &MirrorUpdateOpts, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(&opts.name);
    let mut update = mirror.update();

    if let Some(rename) = &opts.rename {
        update.rename(rename);
    }
    if !opts.keyrings.is_empty() {
        update.keyrings(opts.keyrings.clone());
    }
    if opts.ignore_checksums {
        update.ignore_checksums(true);
    }
    if opts.ignore_signatures {
        update.ignore_signatures(true);
    }
    if opts.force_update {
        update.force_update(true);
    }
    if opts.skip_existing_packages {
        update.skip_existing_packages(true);
    }
    if opts.latest_only {
        update.latest_only(true);
    }

    update.run().await?;

    Ok(())
}

#[derive(Parser, Debug)]
pub struct MirrorEditOpts {
    name: String,
    #[clap(long)]
    archive_url: Option<Url>,
    #[clap(long)]
    filter: Option<String>,
    #[clap(long = "architecture")]
    architectures: Vec<String>,
    #[clap(long = "keyring")]
    keyrings: Vec<String>,
    #[clap(long)]
    filter_with_deps: Option<bool>,
    #[clap(long)]
    download_sources: Option<bool>,
    #[clap(long)]
    download_udebs: Option<bool>,
    #[clap(long)]
    download_installer: Option<bool>,
    #[clap(long)]
    ignore_signatures: Option<bool>,
}

async fn edit(opts: &MirrorEditOpts, aptly: &AptlyRest) -> Result<()> {
    let mirror = aptly.mirror(&opts.name);
    let mut edit = mirror.edit();

    if let Some(archive_url) = &opts.archive_url {
        edit.archive_url(archive_url.to_string());
    }
    if let Some(filter) = &opts.filter {
        edit.filter(filter);
    }
    if !opts.architectures.is_empty() {
        edit.architectures(opts.architectures.clone());
    }
    if !opts.keyrings.is_empty() {
        edit.keyrings(opts.keyrings.clone());
    }
    if let Some(v) = opts.filter_with_deps {
        edit.filter_with_deps(v);
    }
    if let Some(v) = opts.download_sources {
        edit.download_sources(v);
    }
    if let Some(v) = opts.download_udebs {
        edit.download_udebs(v);
    }
    if let Some(v) = opts.download_installer {
        edit.download_installer(v);
    }
    if let Some(v) = opts.ignore_signatures {
        edit.ignore_signatures(v);
    }

    edit.run().await?;

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
    Update(MirrorUpdateOpts),
    Edit(MirrorEditOpts),
    Drop { name: String },
}

impl MirrorCommand {
    pub async fn run(&self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            MirrorCommand::List(args) => list(args.format, aptly).await?,
            MirrorCommand::Create(args) => create_mirror(args, aptly).await?,
            MirrorCommand::Update(args) => update(args, aptly).await?,
            MirrorCommand::Edit(args) => edit(args, aptly).await?,
            MirrorCommand::Drop { name } => drop_mirror(name, aptly).await?,
        }
        Ok(ExitCode::SUCCESS)
    }
}
