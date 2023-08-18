use color_eyre::{eyre::bail, Result};
use debian_packaging::{
    deb::reader::{BinaryPackageEntry, BinaryPackageReader, ControlTarFile},
    package_version::PackageVersion,
};
use futures::TryStreamExt;
use std::path::{Path, PathBuf};
use sync2aptly::{
    AptlyContent, LazyVersion, OriginContent, OriginContentBuilder, OriginDeb, OriginDsc,
    OriginLocation, PackageName, PoolPackagesCache, SyncActions,
};
use tracing::warn;

use aptly_rest::{
    changes::{Changes, ChangesFile},
    dsc::Dsc,
    key::AptlyKey,
    utils::scanner::{self, Scanner},
    AptlyRest,
};

#[tracing::instrument]
fn origin_deb_version(path: &Path) -> Result<PackageVersion> {
    let f = std::fs::File::open(path)?;
    let mut parser = BinaryPackageReader::new(f)?;

    while let Some(entry) = parser.next_entry() {
        if let Ok(BinaryPackageEntry::Control(mut control)) = entry {
            let entries = control.entries()?;
            for e in entries {
                let mut e = e?;
                if let (_, ControlTarFile::Control(c)) = e.to_control_file()? {
                    return c.version().map_err(|e| e.into());
                }
            }
        }
    }

    bail!("Version not found in {}", path.display());
}

#[tracing::instrument(skip_all, fields(changes = ?changes.path(), f = f.name))]
fn origin_deb_for_changes_file(changes: &Changes, f: &ChangesFile) -> Result<OriginDeb> {
    let info = f.parse_name()?;
    let path = changes.path().with_file_name(&f.name);
    let package_name: PackageName = info.package.into();

    let path2 = path.clone();
    let version = LazyVersion::new(Box::new(move || origin_deb_version(&path2)));

    Ok(OriginDeb {
        package: package_name,
        version,
        architecture: info.architecture.to_owned(),
        location: OriginLocation::Path(path),
        from_source: changes.source()?.to_owned().into(),
        aptly_hash: f.aptly_hash(),
    })
}

#[tracing::instrument(skip_all, fields(?dsc = dsc.path()))]
fn origin_dsc_for_aptly_dsc(dsc: &Dsc) -> Result<OriginDsc> {
    let a: AptlyKey = dsc.try_into()?;
    let package: PackageName = a.package().into();
    Ok(OriginDsc {
        package,
        version: dsc.dsc().version()?,
        dsc_location: OriginLocation::Path(dsc.path().to_owned()),
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

#[tracing::instrument(skip_all)]
pub async fn sync(
    obs_path: PathBuf,
    aptly: AptlyRest,
    aptly_content: AptlyContent,
    pool_packages: PoolPackagesCache,
) -> Result<SyncActions> {
    let origin_content = scan_content(obs_path).await?;
    sync2aptly::sync(origin_content, aptly, aptly_content, pool_packages).await
}
