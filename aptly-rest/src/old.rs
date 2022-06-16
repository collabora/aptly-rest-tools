#![allow(dead_code)]
use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use debian_packaging::package_version::PackageVersion;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct Source {
    package: String,
    version: String,
    key: String,
    #[serde(rename = "Checksums-Sha256")]
    sha256: String,
    #[serde(flatten)]
    _unparsed: serde_json::Value,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct Binary {
    package: String,
    version: String,
    architecture: String,
    key: String,
    #[serde(rename = "SHA256")]
    sha256: String,
    #[serde(flatten)]
    _unparsed: serde_json::Value,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum Package {
    Binary(Binary),
    Source(Source),
}

impl Package {
    fn package(&self) -> &str {
        match self {
            Package::Binary(b) => &b.package,
            Package::Source(s) => &s.package,
        }
    }

    fn key(&self) -> &str {
        match self {
            Package::Binary(b) => &b.key,
            Package::Source(s) => &s.key,
        }
    }

    fn version(&self) -> &str {
        match self {
            Package::Binary(b) => &b.version,
            Package::Source(s) => &s.version,
        }
    }

    fn sha256(&self) -> &str {
        match self {
            Package::Binary(b) => &b.sha256,
            Package::Source(s) => &s.sha256,
        }
    }

    fn is_source(&self) -> bool {
        matches!(self, Package::Source(_))
    }
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Package::Binary(b) => write!(f, "{} {} {}", b.package, b.version, b.architecture),
            Package::Source(s) => write!(f, "{} {} source", s.package, s.version),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
struct Repo {
    name: String,
    #[serde(flatten)]
    unparsed: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct PackageRefs<'a> {
    package_refs: &'a [&'a str],
}

struct Client {
    client: reqwest::Client,
    url: Url,
}

impl Client {
    pub fn new(url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
        }
    }

    pub async fn repos(&self) -> Result<Vec<Repo>> {
        let mut u = self.url.clone();
        u.path_segments_mut().unwrap().extend(["api", "repos"]);

        let r = self.client.get(u).send().await?.error_for_status()?;

        Ok(r.json().await?)
    }

    pub async fn packages(&self, repo: &str) -> Result<Vec<Package>> {
        let mut u = self.url.clone();
        u.path_segments_mut()
            .unwrap()
            .extend(["api", "repos", repo, "packages"]);
        u.query_pairs_mut().append_pair("format", "details");

        let r = self.client.get(u).send().await?.error_for_status()?;

        Ok(r.json().await?)
    }

    pub async fn include_packages_by_key(&self, repo: &str, package_refs: &[&str]) -> Result<()> {
        let refs = PackageRefs { package_refs };
        let mut u = self.url.clone();
        u.path_segments_mut()
            .unwrap()
            .extend(["api", "repos", repo, "packages"]);

        let _r = self
            .client
            .post(u)
            .json(&refs)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn delete_packages_by_key(&self, repo: &str, package_refs: &[&str]) -> Result<()> {
        let refs = PackageRefs { package_refs };
        let mut u = self.url.clone();
        u.path_segments_mut()
            .unwrap()
            .extend(["api", "repos", repo, "packages"]);

        let _r = self
            .client
            .delete(u)
            .json(&refs)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum PackageKey {
    Source {
        package: String,
        version: String,
    },
    Binary {
        package: String,
        version: String,
        architecture: String,
    },
}

impl From<&Package> for PackageKey {
    fn from(p: &Package) -> Self {
        match p {
            Package::Binary(b) => PackageKey::Binary {
                package: b.package.clone(),
                architecture: b.architecture.clone(),
                version: b.version.clone(),
            },
            Package::Source(s) => PackageKey::Source {
                package: s.package.clone(),
                version: s.version.clone(),
            },
        }
    }
}

fn should_be_replaced<'a>(
    package: &Package,
    canonical: &'a HashMap<PackageKey, Package>,
) -> Result<Option<&'a Package>> {
    let key = package.into();
    let entry = canonical.get(&key);
    if let Some(entry) = entry {
        if package.sha256() != entry.sha256() {
            println!(
                "Mismatch: {} - {} <> {}",
                package,
                package.sha256(),
                entry.sha256()
            );
            return Ok(Some(entry));
        }
    } else {
        let old = canonical.values().find(|o| {
            if o.package() != package.package() {
                false
            } else {
                match (&o, package) {
                    (Package::Source(_), Package::Source(_)) => true,
                    (Package::Binary(ob), Package::Binary(pb)) => {
                        ob.architecture == pb.architecture
                    }
                    _ => false,
                }
            }
        });
        if let Some(old) = old {
            let old_v = PackageVersion::parse(old.version())?;
            let p_v = PackageVersion::parse(package.version())?;

            if old_v < p_v {
                println!("Newer: {} -> {}", old, package);
            } else {
                println!("Older: {} -> {}", old, package);
                return Ok(Some(old));
            }
        } else {
            println!("New: {}", package);
        }
    }
    Ok(None)
}

#[derive(Parser, Debug)]
struct Opts {
    #[clap(short, long)]
    replace: bool,
    canonical: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    let c = Client::new("http://localhost:8080".try_into()?);

    let packages = c.packages(&opts.canonical).await?;
    let map: HashMap<_, _> = packages
        .into_iter()
        .map(|p| (PackageKey::from(&p), p))
        .collect();

    let repos = c.repos().await?;

    for r in &repos {
        if r.name == opts.canonical {
            continue;
        }
        println!("== {} ==", r.name);
        let packages = c.packages(&r.name).await?;
        for p in packages {
            if let Some(replacement) = should_be_replaced(&p, &map)? {
                if opts.replace {
                    println!(" => Replacing \"{}\" => \"{}\"", p.key(), replacement.key());
                    /* If it's a replacement of the *same* version, remove first then add the new
                     * one to avoid aptly being unhappy; Otherwise do the reverse for safety
                     */
                    if p.version() == replacement.version() {
                        c.delete_packages_by_key(&r.name, &[p.key()]).await?;
                        c.include_packages_by_key(&r.name, &[replacement.key()])
                            .await?;
                    } else {
                        c.include_packages_by_key(&r.name, &[replacement.key()])
                            .await?;
                        c.delete_packages_by_key(&r.name, &[p.key()]).await?;
                    }
                } else {
                    println!(
                        " => Would replace \"{}\" => \"{}\"",
                        p.key(),
                        replacement.key()
                    );
                }
            }
        }
    }

    Ok(())
}
