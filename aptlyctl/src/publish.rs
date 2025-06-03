use std::{io::stdout, process::ExitCode};

use aptly_rest::{api::publish, AptlyRest};
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use tracing::{debug, info};

use crate::OutputFormat;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum SourceKind {
    Repo,
    Snapshot,
}

impl From<SourceKind> for publish::SourceKind {
    fn from(from: SourceKind) -> Self {
        match from {
            SourceKind::Repo => Self::Local,
            SourceKind::Snapshot => Self::Snapshot,
        }
    }
}

fn parse_source(s: &str) -> Result<publish::Source, Box<dyn std::error::Error + Send + Sync>> {
    if let Some((name, component)) = s.split_once("//") {
        Ok(publish::Source {
            name: name.to_owned(),
            component: Some(component.to_owned()),
        })
    } else {
        Ok(publish::Source {
            name: s.to_owned(),
            component: None,
        })
    }
}

#[derive(Parser, Debug)]
pub struct PublishCreateOpts {
    kind: SourceKind,
    prefix: String,
    #[clap(value_parser = parse_source)]
    sources: Vec<publish::Source>,
    #[clap(long = "architecture")]
    architectures: Vec<String>,
    #[clap(long)]
    distribution: Option<String>,
    #[clap(long)]
    gpg_key: Option<String>,
    #[clap(long)]
    skip_bz2: bool,
    #[clap(long)]
    skip_contents: bool,
}

#[derive(Parser, Debug)]
pub struct PublishListOpts {
    #[clap(long, value_enum, default_value_t)]
    format: OutputFormat,
}

#[derive(Parser, Debug)]
pub struct PublishTestExistsOpts {
    prefix: String,
    distribution: String,
}

#[derive(Parser, Debug)]
pub struct PublishUpdateOpts {
    prefix: String,
    distribution: String,
    #[clap(long)]
    gpg_key: Option<String>,
    #[clap(long)]
    skip_bz2: bool,
    #[clap(long)]
    skip_contents: bool,
}

#[derive(Parser, Debug)]
pub struct PublishDropOpts {
    prefix: String,
    distribution: String,
    #[clap(long)]
    force: bool,
    #[clap(long)]
    ignore_if_missing: bool,
}

#[derive(Subcommand, Debug)]
pub enum PublishCommand {
    Create(PublishCreateOpts),
    List(PublishListOpts),
    TestExists(PublishTestExistsOpts),
    Update(PublishUpdateOpts),
    Drop(PublishDropOpts),
}

impl PublishCommand {
    pub async fn run(self, aptly: &AptlyRest) -> Result<ExitCode> {
        match self {
            PublishCommand::Create(args) => {
                let signing = if let Some(key) = args.gpg_key {
                    publish::Signing::Enabled(publish::SigningOptions {
                        gpg_key: Some(key),
                        ..Default::default()
                    })
                } else {
                    publish::Signing::Disabled
                };

                let repo = aptly
                    .publish_prefix(&args.prefix)
                    .publish(
                        args.kind.into(),
                        &args.sources,
                        &publish::PublishOptions {
                            architectures: args.architectures,
                            distribution: args.distribution,
                            signing: Some(signing),
                            skip_bz2: args.skip_bz2,
                            skip_contents: args.skip_contents,
                            ..Default::default()
                        },
                    )
                    .await?;
                debug!(?repo);
                info!("Created new published repository at '{}'", repo.prefix());
            }
            PublishCommand::List(args) => {
                let publishes = aptly.published().await?;

                match args.format {
                    OutputFormat::Name => {
                        let mut names: Vec<_> = publishes
                            .iter()
                            .map(|p| format!("{} {}", p.prefix(), p.distribution()))
                            .collect();
                        names.sort();
                        for name in names {
                            println!("{}", name);
                        }
                    }
                    OutputFormat::Json => {
                        serde_json::to_writer_pretty(&mut stdout(), &publishes)?;
                        println!();
                    }
                }
            }
            PublishCommand::TestExists(args) => {
                let publishes = aptly.published().await?;
                if !publishes
                    .iter()
                    .any(|p| p.prefix() == args.prefix && p.distribution() == args.distribution)
                {
                    return Ok(ExitCode::FAILURE);
                }
            }
            PublishCommand::Update(args) => {
                let signing = if let Some(key) = args.gpg_key {
                    publish::Signing::Enabled(publish::SigningOptions {
                        gpg_key: Some(key),
                        ..Default::default()
                    })
                } else {
                    publish::Signing::Disabled
                };

                let repo = aptly
                    .publish_prefix(&args.prefix)
                    .distribution(&args.distribution)
                    .update(&publish::UpdateOptions {
                        skip_bz2: args.skip_bz2,
                        skip_contents: args.skip_contents,
                        signing: Some(signing),
                        ..Default::default()
                    })
                    .await?;
                debug!(?repo);
                info!(
                    "Updated published repository at '{}/{}'",
                    repo.prefix(),
                    repo.distribution()
                );
            }
            PublishCommand::Drop(args) => {
                if args.ignore_if_missing
                    && !aptly
                        .published()
                        .await?
                        .into_iter()
                        .any(|p| p.prefix() == args.prefix && p.distribution() == args.distribution)
                {
                    info!("Not published; doing nothing.");
                } else {
                    aptly
                        .publish_prefix(&args.prefix)
                        .distribution(&args.distribution)
                        .delete(&publish::DeleteOptions { force: args.force })
                        .await?;
                    info!(
                        "Deleted published repository at '{}/{}'",
                        args.prefix, args.distribution
                    );
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
