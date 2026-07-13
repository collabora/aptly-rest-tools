use std::{
    fs,
    io::{stdin, stdout, Read},
    path::PathBuf,
    process::ExitCode,
};

use aptly_rest::{api::publish, AptlyRest};
use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::Result;
use tracing::{debug, info, warn};

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
    skip_cleanup: bool,
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

#[derive(Parser, Debug)]
pub struct PublishImportOpts {
    /// Path to the file containing publish definitions (reads from stdin if not provided)
    file: Option<PathBuf>,
    /// Skip published repositories that already exist
    #[clap(long)]
    skip_existing: bool,
    /// Input format (json or yaml). If not specified, will be inferred from file extension
    #[clap(long, value_enum)]
    format: Option<OutputFormat>,
}

#[derive(Subcommand, Debug)]
pub enum PublishCommand {
    Create(PublishCreateOpts),
    List(PublishListOpts),
    Import(PublishImportOpts),
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
                    OutputFormat::Yaml => {
                        serde_yaml::to_writer(&mut stdout(), &publishes)?;
                    }
                }
            }
            PublishCommand::Import(args) => {
                let content = if let Some(file) = &args.file {
                    fs::read(file)?
                } else {
                    let mut buffer = Vec::<u8>::new();
                    stdin().read_to_end(&mut buffer)?;
                    buffer
                };

                let publishes: Vec<publish::PublishedRepo> = match args.format {
                    Some(OutputFormat::Json) => serde_json::from_slice(&content)?,
                    Some(OutputFormat::Yaml) => serde_yaml::from_slice(&content)?,
                    Some(OutputFormat::Name) => {
                        return Err(color_eyre::eyre::eyre!(
                            "Name format is not supported for import"
                        ));
                    }
                    None => {
                        // Infer from file extension
                        if let Some(file) = &args.file {
                            match file.extension().and_then(|e| e.to_str()) {
                                Some("yaml" | "yml") => serde_yaml::from_slice(&content)?,
                                Some("json") => serde_json::from_slice(&content)?,
                                _ => {
                                    return Err(color_eyre::eyre::eyre!("Unsupported file format"));
                                }
                            }
                        } else {
                            // Parse stdin always as JSON
                            serde_json::from_slice(&content)?
                        }
                    }
                };

                let existing = aptly.published().await?;

                let conflicts: Vec<_> = publishes
                    .iter()
                    .filter(|p| {
                        existing.iter().any(|e| {
                            e.prefix() == p.prefix() && e.distribution() == p.distribution()
                        })
                    })
                    .collect();

                if !conflicts.is_empty() && !args.skip_existing {
                    for p in &conflicts {
                        warn!(
                            "Published repository '{}/{}' already exists",
                            p.prefix(),
                            p.distribution()
                        );
                    }
                    return Err(color_eyre::eyre::eyre!(
                        "{} published repositories already exist; use --skip-existing to skip them",
                        conflicts.len()
                    ));
                }

                let mut created = 0;
                let mut skipped = 0;

                for repo in publishes {
                    if existing.iter().any(|e| {
                        e.prefix() == repo.prefix() && e.distribution() == repo.distribution()
                    }) {
                        info!(
                            "Published repository '{}/{}' already exists, skipping",
                            repo.prefix(),
                            repo.distribution()
                        );
                        skipped += 1;
                    } else {
                        aptly
                            .publish_prefix(repo.prefix())
                            .publish(
                                repo.source_kind(),
                                repo.sources(),
                                &publish::PublishOptions {
                                    architectures: repo.architectures().to_vec(),
                                    distribution: Some(repo.distribution().to_owned()),
                                    label: Some(repo.label().to_owned()),
                                    origin: Some(repo.origin().to_owned()),
                                    not_automatic: repo.not_automatic(),
                                    but_automatic_upgrades: repo.but_automatic_upgrades(),
                                    acquire_by_hash: repo.acquire_by_hash(),
                                    skip_contents: repo.skip_contents(),
                                    ..Default::default()
                                },
                            )
                            .await?;
                        debug!(?repo);
                        info!("Created new published repository at '{}'", repo.prefix());
                        created += 1;
                    }
                }

                info!("Import complete: {} created, {} skipped", created, skipped);
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
                        skip_cleanup: args.skip_cleanup,
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
