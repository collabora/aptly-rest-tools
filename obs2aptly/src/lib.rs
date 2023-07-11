use color_eyre::{eyre::ensure, Result};
use debian_packaging::package_version::PackageVersion;
use futures::TryStreamExt;
use std::{collections::HashMap, path::PathBuf};
use sync2aptly::{
    AddDebOptions, AptlyContent, AptlyPackage, MatchPoolPackageBy, OriginContent,
    OriginContentBuilder, OriginDeb, OriginDsc, OriginPackage, OriginSource, PackageName,
    SyncActions, Syncer, Syncers,
};
use tracing::{info, warn};

use aptly_rest::{
    changes::{Changes, ChangesFile},
    dsc::Dsc,
    key::AptlyKey,
    utils::scanner::{self, Scanner},
    AptlyRest,
};

#[tracing::instrument(skip_all, fields(changes = ?changes.path(), f = f.name))]
fn origin_deb_for_changes_file(changes: &Changes, f: &ChangesFile) -> Result<OriginDeb> {
    let info = f.parse_name()?;
    let package_name: PackageName = info.package.into();
    Ok(OriginDeb {
        package: package_name,
        architecture: info.architecture.to_owned(),
        path: changes.path().with_file_name(&f.name),
        from_source: Some((changes.source()?.to_owned().into(), changes.version()?)),
        aptly_hash: f.aptly_hash(),
    })
}

#[tracing::instrument(skip_all, fields(?dsc = dsc.path()))]
fn origin_dsc_for_aptly_dsc(dsc: &Dsc) -> Result<OriginDsc> {
    let a: AptlyKey = dsc.try_into()?;
    let package: PackageName = a.package().into();
    Ok(OriginDsc {
        package,
        dsc_path: dsc.path().to_owned(),
        files: dsc.files()?,
        aptly_hash: a.hash().to_string(),
    })
}

#[tracing::instrument]
async fn scan_content(path: PathBuf) -> Result<OriginContent> {
    let mut builder = OriginContentBuilder::new();

    let mut scanner = Scanner::new(path);

    while let Some(control) = scanner.try_next().await? {
        match control {
            scanner::Found::Changes(changes) => {
                for f in changes.files()? {
                    if !f.name.ends_with(".deb") && !f.name.ends_with(".udeb") {
                        continue;
                    }

                    builder.add_deb(origin_deb_for_changes_file(&changes, &f)?);
                }
            }
            scanner::Found::Dsc(dsc) => {
                builder.add_dsc(origin_dsc_for_aptly_dsc(&dsc)?);
            }
        }
    }

    Ok(builder.build())
}

pub struct BinaryDepSyncer;

#[async_trait::async_trait]
impl Syncer for BinaryDepSyncer {
    type Origin = OriginPackage;

    #[tracing::instrument(skip_all)]
    async fn add(
        &self,
        _name: &PackageName,
        origin: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()> {
        let origin_newest = origin.newest()?;
        actions.add_deb(origin_newest);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn sync(
        &self,
        name: &PackageName,
        origin: &Self::Origin,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // Simple case just one package on both sides
        let origin_newest = origin.newest()?;
        let aptly_newest = aptly.newest()?;
        if &origin_newest.version()? < aptly_newest.version() {
            warn!("{} older than {} in aptly", origin_newest, aptly_newest);
        } else if origin_newest.aptly_hash != aptly_newest.hash() {
            info!("== Changes for {} ==", name);
            for key in aptly.keys().cloned() {
                actions.remove_aptly(key);
            }
            actions.add_deb(origin_newest);
        }
        Ok(())
    }
}

struct BinaryInDepSyncer;

#[async_trait::async_trait]
impl Syncer for BinaryInDepSyncer {
    type Origin = OriginPackage;

    #[tracing::instrument(skip_all)]
    async fn add(
        &self,
        _name: &PackageName,
        obs: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // Only add the newest arch all package; Potential future update could be to add one deb
        // for each *version*
        let obs_newest = obs.newest()?;
        actions.add_deb_with_options(
            obs_newest,
            &AddDebOptions {
                match_existing: MatchPoolPackageBy::KeyOrFilename,
            },
        );
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn sync(
        &self,
        name: &PackageName,
        obs: &Self::Origin,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // Common case,  all arch all are from a single source package so what should be published
        // is a package equal to a version of the newest source package version.
        //
        // cornercase; arch all package is provided by multiple source packages; e.g. during a
        // transition.
        //
        // Only put newer stuff in aptly; but then build id's?
        // sort per version
        // aptly should have one arch all for each version
        //
        // sort by (source, version) => all package
        //   for each (source, versionj) check for a matching package in aptly
        //     -> common fast if all source packages are the same
        //
        //   if match not found check if source version is *newer* then all aptly version; if
        //   so add it
        //      -> common when doing a package update
        //
        //   if not fall back checking the exact version
        //   get the exact version of the .deb as it may have an epoch which means it's newer
        //     -> shortcut if aptly has no epoch and changes version matches?
        //
        info!("=== Changes for {} ===", name);
        let mut keep_in_aptly = Vec::new();

        let obs_by_version: HashMap<&(PackageName, PackageVersion), Vec<&OriginDeb>> =
            obs.debs().iter().fold(HashMap::new(), |mut acc, d| {
                acc.entry(d.from_source.as_ref().unwrap())
                    .or_default()
                    .push(d);
                acc
            });

        for ((source_name, source_version), v) in obs_by_version.iter() {
            info!(
                "=== Changes for source {} - {} ===",
                source_name, source_version
            );
            // If there is a matching hash this version already available in aptly so no further
            // action needed
            if let Some(found) = v
                .iter()
                .find_map(|p| aptly.keys().find(|a| a.hash() == p.aptly_hash))
            {
                info!("Keeping {} as it matches a hash in OBS", found);
                keep_in_aptly.push(found);
                continue;
            }

            // Assuming all builds from the same source version will have the same version of the
            // package
            let deb_version = &v[0].version()?;
            // If version is newer then everything in aptly add the package
            if aptly.keys().all(|a| a.version() < deb_version) {
                actions.add_deb_with_options(
                    v[0],
                    &AddDebOptions {
                        match_existing: MatchPoolPackageBy::KeyOrFilename,
                    },
                );
                continue;
            }

            // If any of the aptly keys match the exact package version, then happyness?
            if let Some(found) = aptly.keys().find(|a| a.version() == deb_version) {
                info!("Keeping {} as it matches a version in OBS", found);
                keep_in_aptly.push(found);
            }
        }

        let obs_newest = obs.newest()?.version()?;
        for a in aptly.keys().filter(|key| !keep_in_aptly.contains(key)) {
            if a.version() < &obs_newest {
                info!("Removing {}", a);
                actions.remove_aptly(a.clone());
            } else {
                info!("Keeping {} as it was newer then anything in OBS", a);
            }
        }

        Ok(())
    }
}

pub struct SourceSyncer;

#[async_trait::async_trait]
impl Syncer for SourceSyncer {
    type Origin = OriginSource;

    #[tracing::instrument(skip_all)]
    async fn add(
        &self,
        _name: &PackageName,
        obs: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()> {
        for source in obs.sources() {
            actions.add_dsc(source)?;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn sync(
        &self,
        _name: &PackageName,
        origin: &Self::Origin,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // TODO let aptly keep all source version referred to by changes files? Though this would
        // need to account for build suffixes in some way

        // Simple case just one package on both sides
        let d = &origin.sources()[0];
        // If there are multiple source files, make sure their hashes are fully identical.
        ensure!(
            origin
                .sources()
                .iter()
                .skip(1)
                .all(|s| s.aptly_hash == d.aptly_hash),
            "Multiple sources of different hashes for a single package: {:?}",
            origin
                .sources()
                .iter()
                .map(|s| s.dsc_path.display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );

        let a = aptly.keys().next().unwrap();

        if d.aptly_hash != a.hash() {
            // TODO make sure version is upgraded
            actions.remove_aptly(a.clone());
            actions.add_dsc(d)?;
        }

        Ok(())
    }
}

#[tracing::instrument(skip_all)]
pub async fn sync(
    obs_path: PathBuf,
    aptly: AptlyRest,
    aptly_content: AptlyContent,
) -> Result<SyncActions> {
    let origin_content = scan_content(obs_path).await?;
    sync2aptly::sync(
        origin_content,
        aptly,
        aptly_content,
        &mut Syncers {
            binary_dep: BinaryDepSyncer,
            binary_indep: BinaryInDepSyncer,
            source: SourceSyncer,
        },
    )
    .await
}
