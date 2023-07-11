use color_eyre::{
    eyre::{ensure, eyre},
    Result,
};
use debian_packaging::{
    binary_package_control::BinaryPackageControlFile,
    control::ControlParagraphAsyncReader,
    debian_source_control::DebianSourceControlFile,
    error::DebianError,
    io::Compression,
    repository::{
        builder::DebPackageReference,
        filesystem::FilesystemRepositoryReader,
        release::{ChecksumType, ReleaseFileEntry},
        ReleaseReader, RepositoryRootReader,
    },
};
use futures::io::{AsyncBufRead, BufReader as AsyncBufReader};
use std::{collections::HashMap, path::Path};
use sync2aptly::{
    AptlyContent, AptlyPackage, OriginContent, OriginContentBuilder, OriginDeb, OriginDsc,
    OriginPackage, OriginSource, PackageName, SyncActions, Syncer, Syncers,
};
use tracing::{info, info_span, warn};

use aptly_rest::{
    dsc::DscFile,
    key::{AptlyHashBuilder, AptlyHashFile},
    AptlyRest,
};

#[tracing::instrument(skip_all)]
async fn entry_reader(
    release: &dyn ReleaseReader,
    entry: &ReleaseFileEntry<'_>,
) -> Result<ControlParagraphAsyncReader<impl AsyncBufRead>> {
    Ok(ControlParagraphAsyncReader::new(AsyncBufReader::new(
        release
            .get_path_with_digest_verification(entry.path, entry.size, entry.digest.clone())
            .await?,
    )))
}

#[tracing::instrument]
fn basename_or_error(path: &str) -> Result<&str> {
    path.split('/')
        .last()
        .ok_or_else(|| eyre!("Bad filename {path}"))
}

#[tracing::instrument(skip(builder, release))]
async fn scan_dist_packages(
    builder: &mut OriginContentBuilder,
    root_path: &Path,
    release: &dyn ReleaseReader,
    component: &str,
    arch: &str,
) -> Result<()> {
    info!("Scanning packages");

    let entry = match release.packages_entry(component, arch, false) {
        Ok(entry) => entry,
        Err(DebianError::RepositoryReadPackagesIndicesEntryNotFound) => {
            info!("Skipping missing entry");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let mut reader = entry_reader(release, &entry).await?;
    while let Some(paragraph) = reader.read_paragraph().await? {
        let bin = BinaryPackageControlFile::from(paragraph);
        let package = bin.package()?.into();

        let span = info_span!("scan_dist_packages:package", ?package);
        let _enter = span.enter();

        let filename = bin.required_field_str("Filename")?;

        let aptly_hash = AptlyHashBuilder::default()
            .file(&AptlyHashFile {
                basename: basename_or_error(filename)?,
                size: bin.size().ok_or_else(|| eyre!("Missing Size field"))??,
                md5: &bin.deb_digest(ChecksumType::Md5)?.digest_hex(),
                sha1: &bin.deb_digest(ChecksumType::Sha1)?.digest_hex(),
                sha256: &bin.deb_digest(ChecksumType::Sha256)?.digest_hex(),
            })
            .finish();

        builder.add_deb(OriginDeb {
            package,
            architecture: bin.architecture()?.to_owned(),
            path: root_path.join(filename),
            from_source: None,
            aptly_hash,
        });
    }

    Ok(())
}

#[tracing::instrument(skip(source))]
fn collect_source_files(source: &DebianSourceControlFile<'_>) -> Result<Vec<DscFile>> {
    let md5_entries = source.files()?.collect::<Result<Vec<_>, _>>()?;
    let sha1_entries = source
        .checksums_sha1()
        .ok_or_else(|| eyre!("Missing Checksums-Sha1"))?
        .collect::<Result<Vec<_>, _>>()?;
    let sha256_entries = source
        .checksums_sha256()
        .ok_or_else(|| eyre!("Missing Checksums-Sha256"))?
        .collect::<Result<Vec<_>, _>>()?;

    ensure!(
        md5_entries.len() == sha1_entries.len() && md5_entries.len() == sha256_entries.len(),
        "MD5, SHA1, SHA256 do not have matching files"
    );

    (0..md5_entries.len())
        .map(|i| {
            let md5 = &md5_entries[i];
            let sha1 = &sha1_entries[i];
            let sha256 = &sha256_entries[i];

            ensure!(
                md5.filename == sha1.filename
                    && md5.filename == sha256.filename
                    && md5.size == sha1.size
                    && md5.size == sha256.size,
                "Files mismatch: md5={} {}, sha1={} {}, sha256={} {}",
                md5.filename,
                md5.size,
                sha1.filename,
                sha1.size,
                sha256.filename,
                sha256.size,
            );
            ensure!(
                md5.filename.matches('/').count() == 0,
                "Invalid basename {}",
                md5.filename
            );

            Ok(DscFile {
                name: md5.filename.to_owned(),
                size: md5.size,
                md5: md5.digest.digest_hex(),
                sha1: sha1.digest.digest_hex(),
                sha256: sha256.digest.digest_hex(),
            })
        })
        .collect()
}

#[tracing::instrument]
fn find_dsc_file(files: &[DscFile]) -> Result<&DscFile> {
    let dsc_files: Vec<_> = files.iter().filter(|f| f.name.ends_with(".dsc")).collect();
    ensure!(dsc_files.len() == 1, "Expected 1 .dsc file");
    Ok(dsc_files[0])
}

#[tracing::instrument(skip(builder, release))]
async fn scan_dist_sources(
    builder: &mut OriginContentBuilder,
    root_path: &Path,
    release: &dyn ReleaseReader,
    component: &str,
) -> Result<()> {
    info!("Scanning sources");

    let entry = release.sources_entry(component)?;
    let mut reader = entry_reader(release, &entry).await?;
    while let Some(paragraph) = reader.read_paragraph().await? {
        let source = DebianSourceControlFile::from(paragraph);
        let package = source
            .source()
            .or_else(|_| source.required_field_str("Package"))
            .map_err(|_| eyre!("Missing Source/Package field"))?
            .into();

        let span = info_span!("scan_dist_sources:package", ?package);
        let _enter = span.enter();

        let files = collect_source_files(&source)?;
        let dsc = find_dsc_file(&files)?;

        let mut aptly_hash_builder = AptlyHashBuilder::default();
        for file in &files {
            aptly_hash_builder.add_file(&AptlyHashFile {
                basename: &file.name,
                size: file.size,
                md5: &file.md5,
                sha1: &file.sha1,
                sha256: &file.sha256,
            });
        }

        builder.add_dsc(OriginDsc {
            package,
            dsc_path: root_path
                .join(source.required_field_str("Directory")?)
                .join(&dsc.name),
            files,
            aptly_hash: aptly_hash_builder.finish(),
        });
    }

    Ok(())
}

#[tracing::instrument]
async fn scan_dist(root_path: &Path, dist: &str) -> Result<OriginContent> {
    let mut builder = OriginContentBuilder::new();

    let root = FilesystemRepositoryReader::new(root_path);
    let mut release = root.release_reader(dist).await?;

    // Don't use compression, because this is running off the local disk anyway
    // & it simplifies some of the code.
    release.set_preferred_compression(Compression::None);

    let architectures = release
        .release_file()
        .architectures()
        .ok_or_else(|| eyre!("Release file has no architectures"))?
        .collect::<Vec<_>>();
    for component in release
        .release_file()
        .components()
        .ok_or_else(|| eyre!("Release file has no components"))?
    {
        for arch in &architectures {
            scan_dist_packages(&mut builder, root_path, &*release, component, arch).await?;
        }

        scan_dist_sources(&mut builder, root_path, &*release, component).await?;
    }

    Ok(builder.build())
}

pub struct BinarySyncer;

#[async_trait::async_trait]
impl Syncer for BinarySyncer {
    type Origin = OriginPackage;

    #[tracing::instrument(skip_all)]
    async fn add(
        &self,
        _name: &PackageName,
        origin: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()> {
        for deb in origin
            .debs()
            .iter()
            .map(|d| (&d.aptly_hash, d))
            .collect::<HashMap<_, _>>()
            .values()
        {
            actions.add_deb(deb);
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
        let mut origin_hashes: HashMap<_, _> = origin
            .debs()
            .iter()
            .map(|d| (d.aptly_hash.as_ref(), d))
            .collect();
        for key in aptly.keys().cloned() {
            if origin_hashes.remove(key.hash()).is_none() {
                actions.remove_aptly(key);
            }
        }

        for deb in origin_hashes.values() {
            actions.add_deb(deb);
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
        origin: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()> {
        for source in origin.sources() {
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
        let mut origin_hashes: HashMap<_, _> = origin
            .sources()
            .iter()
            .map(|d| (d.aptly_hash.as_ref(), d))
            .collect();
        for key in aptly.keys().cloned() {
            if origin_hashes.remove(key.hash()).is_none() {
                actions.remove_aptly(key);
            }
        }

        for dsc in origin_hashes.values() {
            actions.add_dsc(dsc)?;
        }

        Ok(())
    }
}

#[tracing::instrument(skip_all)]
pub async fn sync(
    root_path: &Path,
    dist: &str,
    aptly: AptlyRest,
    aptly_content: AptlyContent,
) -> Result<SyncActions> {
    let origin_content = scan_dist(root_path, dist).await?;
    sync2aptly::sync(
        origin_content,
        aptly,
        aptly_content,
        &mut Syncers {
            binary_dep: BinarySyncer,
            binary_indep: BinarySyncer,
            source: SourceSyncer,
        },
    )
    .await
}
