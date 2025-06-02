use std::path::PathBuf;

use anyhow::Result;
use aptly_rest::utils::scanner::{self, Scanner};
use clap::Parser;
use futures::TryStreamExt;

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

#[derive(clap::Parser, Debug)]
struct Opts {
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    scan(opts.path).await?;
    Ok(())
}
