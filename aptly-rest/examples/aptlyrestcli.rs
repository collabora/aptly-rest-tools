use std::path::PathBuf;

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
struct Packages {
    #[clap(short, long)]
    detailed: bool,
    query: Option<String>,
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
    match action {
        RepoAction::Packages(p) => {
            let repo = aptly.repo(name);
            let packages = repo.packages();
            if let Some(query) = p.query {
                let query = packages.query(query, false);
                if p.detailed {
                    for p in query.detailed().await? {
                        println!("{p:#?}");
                    }
                } else {
                    for p in query.list().await? {
                        println!("{p}");
                    }
                }
            } else if p.detailed {
                for p in packages.detailed().await? {
                    println!("{p:#?}");
                }
            } else {
                for p in packages.list().await? {
                    println!("{p}");
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

async fn list_mirrors(aplty: AptlyRest) -> Result<()> {
    let mirrors = aplty.mirrors().await?;

    for m in mirrors {
        println!("* {:?}", m);
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
    Mirrors,
    Repo(Repo),
    Scan(Scan),
}

#[derive(clap::Parser, Debug)]
struct Opts {
    #[clap(short, long, default_value = "http://localhost:8080")]
    url: Url,
    #[clap(subcommand)]
    action: Action,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    let aptly = AptlyRest::new(opts.url);

    match opts.action {
        Action::ParseChanges(p) => parse_changes(p).await?,
        Action::ParseDsc(f) => parse_dsc(f).await?,
        Action::Repos => list_repos(aptly).await?,
        Action::Mirrors => list_mirrors(aptly).await?,
        Action::Repo(r) => repo(r.name, aptly, r.action).await?,
        Action::Scan(s) => scan(s.path).await?,
    }

    Ok(())
}
