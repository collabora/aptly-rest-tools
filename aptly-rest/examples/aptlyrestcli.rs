use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use aptly_rest::{
    changes::Changes,
    dsc::Dsc,
    key::AptlyKey,
    utils::scanner::{self, Scanner},
    AptlyRest,
};
use clap::Parser;
use futures::TryStreamExt;
use reqwest::Url;

#[derive(clap::Parser, Debug)]
struct ParseChanges {
    path: PathBuf,
}

async fn parse_changes(h: ParseChanges) -> Result<()> {
    let changes = Changes::from_file(h.path).await?;
    let files = changes.files()?;

    for _file in files {
        // TODO
        /*
        let key = match AptlyKey::try_from(&file) {
            Ok(key) => key,
            Err(ChangesFileToAptlyKeyError::UnsupportPackageType) => {
                println!("Ignoring unsupported file: {}", file.name);
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        println!("{}", key);
        */
    }

    Ok(())
}

#[derive(clap::Parser, Debug)]
struct HashDsc {
    path: PathBuf,
}

#[derive(clap::Parser, Debug)]
struct ParseDsc {
    path: PathBuf,
}

async fn parse_dsc(h: ParseDsc) -> Result<()> {
    let dsc = Dsc::from_file(h.path).await?;
    let key = AptlyKey::try_from(&dsc)?;
    println!("{}", key);
    Ok(())
}

#[derive(clap::Parser, Debug)]
struct ListPackages {
    #[clap(short, long)]
    detailed: bool,
    query: Option<String>,
}

#[derive(clap::Parser, Debug)]
struct DeletePackages {
    packages: Vec<String>,
}

#[derive(clap::Subcommand, Debug)]
enum PackagesAction {
    List(ListPackages),
    Delete(DeletePackages),
}

#[derive(clap::Parser, Debug)]
struct Packages {
    #[clap(subcommand)]
    action: PackagesAction,
}

#[derive(clap::Subcommand, Debug)]
enum RepoAction {
    Packages(Packages),
}

#[derive(clap::Parser, Debug)]
struct Repo {
    name: String,
    #[clap(subcommand)]
    action: RepoAction,
}

async fn repo(name: String, aptly: AptlyRest, action: RepoAction) -> Result<()> {
    let repo = aptly.repo(name);
    match action {
        RepoAction::Packages(p) => {
            let packages = repo.packages();
            match p.action {
                PackagesAction::List(list) => {
                    if let Some(query) = list.query {
                        let query = packages.query(query, false);
                        if list.detailed {
                            for p in query.detailed().await? {
                                println!("{p:#?}");
                            }
                        } else {
                            for p in query.list().await? {
                                println!("{p}");
                            }
                        }
                    } else if list.detailed {
                        for p in packages.detailed().await? {
                            println!("{p:#?}");
                        }
                    } else {
                        for p in packages.list().await? {
                            println!("{p}");
                        }
                    }
                }
                PackagesAction::Delete(d) => {
                    let to_delete: Vec<_> = d
                        .packages
                        .iter()
                        .map(|p| AptlyKey::from_str(p))
                        .collect::<Result<_, _>>()?;

                    packages.delete(&to_delete).await?
                }
            }
        }
    }

    Ok(())
}

async fn list_repos(aplty: AptlyRest) -> Result<()> {
    let repos = aplty.repos().await?;

    for r in repos {
        println!("* {:?}", r);
    }

    Ok(())
}

#[derive(clap::Parser, Debug)]
struct Scan {
    path: PathBuf,
}

async fn scan(path: PathBuf) -> Result<()> {
    let mut scanner = Scanner::new(path);

    while let Some(control) = scanner.try_next().await? {
        match control {
            scanner::Found::Changes(c) => {
                println!("Changes: {}", c.path().display());
                for f in c.files()? {
                    let path = c.path().with_file_name(f.name);
                    println!("-> {}", path.display());
                    if path.extension().and_then(|o| o.to_str()) == Some("deb") {
                        let f = std::fs::File::open(path)?;
                        let control = debian_packaging::deb::reader::resolve_control_file(f)?;
                        println!("   Version: {}", control.version()?);
                    }
                }
            }
            scanner::Found::Dsc(d) => {
                println!("DSC: {}", d.path().display());
            }
        }
    }

    Ok(())
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    ParseChanges(ParseChanges),
    ParseDsc(ParseDsc),
    Repos,
    Repo(Repo),
    Scan(Scan),
}

#[derive(clap::Parser, Debug)]
struct Opts {
    #[clap(
        short = 'u',
        long,
        env = "APTLY_API_URL",
        default_value = "http://localhost:8080"
    )]
    api_url: Url,
    /// Authentication token for the API
    #[clap(long, env = "APTLY_API_TOKEN")]
    api_token: Option<String>,
    #[clap(subcommand)]
    action: Action,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    let aptly = if let Some(token) = opts.api_token {
        AptlyRest::new_with_token(opts.api_url, &token)?
    } else {
        AptlyRest::new(opts.api_url)
    };

    match opts.action {
        Action::ParseChanges(p) => parse_changes(p).await?,
        Action::ParseDsc(f) => parse_dsc(f).await?,
        Action::Repos => list_repos(aptly).await?,
        Action::Repo(r) => repo(r.name, aptly, r.action).await?,
        Action::Scan(s) => scan(s.path).await?,
    }

    Ok(())
}
