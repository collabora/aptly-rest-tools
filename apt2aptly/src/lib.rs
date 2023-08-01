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
        http::HttpRepositoryClient,
        release::{ChecksumType, ReleaseFileEntry},
        ReleaseReader, RepositoryRootReader,
    },
};
use futures::io::{AsyncBufRead, BufReader as AsyncBufReader};
use sync2aptly::{
    AptlyContent, LazyVersion, OriginContentBuilder, OriginDeb, OriginDsc, OriginLocation,
    PackageName, SyncActions,
};
use tracing::{info, info_span, warn};
use url::Url;

use aptly_rest::{
    dsc::DscFile,
    key::{AptlyHashBuilder, AptlyHashFile},
    AptlyRest,
};

#[tracing::instrument]
fn basename_or_error(path: &str) -> Result<&str> {
    path.split('/')
        .last()
        .ok_or_else(|| eyre!("Bad filename {path}"))
}

#[tracing::instrument(skip(source))]
fn collect_source_files(source: &DebianSourceControlFile<'_>) -> Result<Vec<DscFile>> {
    let mut md5_entries = source.files()?.collect::<Result<Vec<_>, _>>()?;
    let mut sha1_entries = source
        .checksums_sha1()
        .ok_or_else(|| eyre!("Missing Checksums-Sha1"))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut sha256_entries = source
        .checksums_sha256()
        .ok_or_else(|| eyre!("Missing Checksums-Sha256"))?
        .collect::<Result<Vec<_>, _>>()?;

    ensure!(
        md5_entries.len() == sha1_entries.len() && md5_entries.len() == sha256_entries.len(),
        "MD5, SHA1, SHA256 do not have matching files"
    );

    // aptly sorts files before hashing them, so we need to do the same in order
    // to keep it consistent.
    md5_entries.sort_by_key(|e| e.filename);
    sha1_entries.sort_by_key(|e| e.filename);
    sha256_entries.sort_by_key(|e| e.filename);

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

pub struct DistScanner {
    root_location: OriginLocation,
    release: Box<dyn ReleaseReader>,
    components: Vec<String>,
    architectures: Vec<String>,
}

impl DistScanner {
    #[tracing::instrument(fields(root_url = root_url.as_str()), skip(root_url))]
    pub async fn new(root_url: &Url, dist: &str) -> Result<Self> {
        let root_location = OriginLocation::Url(root_url.clone());

        let root = HttpRepositoryClient::new(root_url.clone())?;
        let release = root.release_reader(dist).await?;

        let architectures = release
            .release_file()
            .architectures()
            .ok_or_else(|| eyre!("Release file has no architectures"))?
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let components = release
            .release_file()
            .components()
            .ok_or_else(|| eyre!("Release file has no components"))?
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        Ok(Self {
            root_location,
            release,
            architectures,
            components,
        })
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    #[tracing::instrument(skip_all)]
    async fn entry_reader(
        &self,
        entry: &ReleaseFileEntry<'_>,
        compression: Compression,
    ) -> Result<ControlParagraphAsyncReader<impl AsyncBufRead>> {
        Ok(ControlParagraphAsyncReader::new(AsyncBufReader::new(
            self.release
                .get_path_decoded_with_digest_verification(
                    entry.path,
                    compression,
                    entry.size,
                    entry.digest.clone(),
                )
                .await?,
        )))
    }

    #[tracing::instrument(skip(self, builder, component))]
    async fn scan_packages(
        &self,
        builder: &mut OriginContentBuilder,
        component: &str,
        arch: &str,
    ) -> Result<()> {
        info!("Scanning packages");

        let entry = match self.release.packages_entry(component, arch, false) {
            Ok(entry) => entry,
            Err(DebianError::RepositoryReadPackagesIndicesEntryNotFound) => {
                info!("Skipping missing entry");
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        let mut reader = self.entry_reader(&entry, entry.compression).await?;
        while let Some(paragraph) = reader.read_paragraph().await? {
            let bin = BinaryPackageControlFile::from(paragraph);
            let package: PackageName = bin.package()?.into();

            let span = info_span!("scan_packages:package", ?package);
            let _enter = span.enter();

            let filename = bin.required_field_str("Filename")?;

            let from_source = bin
                .source()
                .map(|s| s.to_owned().into())
                .unwrap_or_else(|| package.clone());

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
                version: LazyVersion::with_value(bin.version()?),
                architecture: bin.architecture()?.to_owned(),
                location: self.root_location.join(filename)?,
                from_source,
                aptly_hash,
            });
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, builder, component))]
    async fn scan_sources(
        &self,
        builder: &mut OriginContentBuilder,
        component: &str,
    ) -> Result<()> {
        info!("Scanning sources");

        let entry = self.release.sources_entry(component)?;
        let mut reader = self.entry_reader(&entry, entry.compression).await?;
        while let Some(paragraph) = reader.read_paragraph().await? {
            let source = DebianSourceControlFile::from(paragraph);
            let package = source
                .source()
                .or_else(|_| source.required_field_str("Package"))
                .map_err(|_| eyre!("Missing Source/Package field"))?
                .into();

            let span = info_span!("scan_sources:package", ?package);
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
                version: source.version()?,
                dsc_location: self
                    .root_location
                    .join(source.required_field_str("Directory")?)?
                    .join(&dsc.name)?,
                files,
                aptly_hash: aptly_hash_builder.finish(),
            });
        }

        Ok(())
    }

    #[tracing::instrument(
        fields(
            root_location = self.root_location.as_url().unwrap().as_str(),
            release = self.release.root_relative_path()),
        skip(self, aptly, aptly_content))]
    pub async fn sync_component(
        &self,
        component: &str,
        aptly: AptlyRest,
        aptly_content: AptlyContent,
    ) -> Result<SyncActions> {
        let mut builder = OriginContentBuilder::new();

        for arch in &self.architectures {
            self.scan_packages(&mut builder, component, arch).await?;
        }

        self.scan_sources(&mut builder, component).await?;

        let origin_content = builder.build();
        sync2aptly::sync(origin_content, aptly, aptly_content).await
    }
}
