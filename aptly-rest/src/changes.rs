use debian_packaging::{
    control::{ControlParagraph, ControlParagraphAsyncReader},
    error::{DebianError, Result as DebianResult},
    package_version::PackageVersion,
};
use futures::io::BufReader;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::key::{AptlyHashBuilder, AptlyHashFile};

#[derive(thiserror::Error, Debug)]
pub enum ChangesError {
    #[error("Missing Files line")]
    MissingFiles,
    #[error("Missing Sha1 checksums line")]
    MissingSha1Checksums,
    #[error("Missing Sha256 checksums line")]
    MissingSha256Checksums,
    #[error("Failed to parse files line")]
    FilesParseError,
    #[error("Failed to parse checksums line")]
    ChecksumsParseError,
    #[error("Inconcistent file list")]
    InconsistentFiles,
    #[error("Missing checksum for some files")]
    MissingChecksum,
    #[error("Parse failure: {0}")]
    Parse(#[from] DebianError),
    #[error("Missing control paragraph")]
    MissingParagraph,
    #[error("IO Error")]
    IO(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub struct Changes {
    path: PathBuf,
    paragraph: ControlParagraph<'static>,
}

impl Changes {
    pub async fn from_file(path: PathBuf) -> Result<Self, ChangesError> {
        let file = File::open(&path).await?;
        let buf = BufReader::new(file.compat());

        let mut reader = ControlParagraphAsyncReader::new(buf);
        let paragraph = reader
            .read_paragraph()
            .await?
            .ok_or(ChangesError::MissingParagraph)?
            .to_owned();
        Ok(Changes { path, paragraph })
    }

    /// The `Source` field.
    pub fn source(&self) -> DebianResult<&str> {
        self.paragraph.required_field_str("Source")
    }

    pub fn version_str(&self) -> DebianResult<&str> {
        self.paragraph.required_field_str("Version")
    }

    pub fn architecture(&self) -> DebianResult<&str> {
        self.paragraph.required_field_str("Architecture")
    }

    /// The `Version` field parsed into a [PackageVersion].
    pub fn version(&self) -> DebianResult<PackageVersion> {
        PackageVersion::parse(self.version_str()?)
    }

    pub fn files(&self) -> Result<Vec<ChangesFile>, ChangesError> {
        let mut files = std::collections::HashMap::new();
        for parts in self
            .paragraph
            .iter_field_lines("Files")
            .ok_or(ChangesError::MissingFiles)?
            .map(changes_files_line)
        {
            let (filename, size, digest) = parts?;
            files.entry(filename).or_insert_with(|| FileData {
                size,
                md5: Some(digest),
                ..Default::default()
            });
        }

        for parts in self
            .paragraph
            .iter_field_lines("Checksums-Sha1")
            .ok_or(ChangesError::MissingSha1Checksums)?
            .map(changes_checksums_line)
        {
            let (filename, _size, digest) = parts?;
            let file = files
                .get_mut(&filename)
                .ok_or(ChangesError::InconsistentFiles)?;
            file.sha1 = Some(digest);
        }

        for parts in self
            .paragraph
            .iter_field_lines("Checksums-Sha256")
            .ok_or(ChangesError::MissingSha256Checksums)?
            .map(changes_checksums_line)
        {
            let (filename, _size, digest) = parts?;
            let file = files
                .get_mut(&filename)
                .ok_or(ChangesError::InconsistentFiles)?;
            file.sha256 = Some(digest);
        }

        files
            .drain()
            .map(|(name, data)| {
                Ok(ChangesFile::new(
                    name,
                    data.size,
                    data.md5.ok_or(ChangesError::MissingChecksum)?,
                    data.sha1.ok_or(ChangesError::MissingChecksum)?,
                    data.sha256.ok_or(ChangesError::MissingChecksum)?,
                ))
            })
            .collect()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ChangesFileNameParseError {
    #[error("Invalid file name")]
    InvalidName,
    #[error("Missing package name")]
    MissingPackage,
    #[error("Missing version")]
    MissingVersion,
    #[error("Missing architecture")]
    MissingArchitecture,
    #[error("Failed to parse version")]
    VersionParseError(#[from] DebianError),
}

#[derive(Clone, Debug)]
pub struct ChangesFileInfo<'a> {
    pub package: &'a str,
    pub version: PackageVersion,
    pub architecture: &'a str,
    pub type_: &'a str,
}

#[derive(Clone, Debug)]
pub struct ChangesFile {
    pub name: String,
    pub size: u64,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

impl ChangesFile {
    pub fn new(name: String, size: u64, md5: String, sha1: String, sha256: String) -> Self {
        Self {
            name,
            size,
            md5,
            sha1,
            sha256,
        }
    }

    pub fn parse_name(&self) -> Result<ChangesFileInfo, ChangesFileNameParseError> {
        let path = Path::new(&self.name);
        let stem = path
            .file_stem()
            .ok_or(ChangesFileNameParseError::InvalidName)?;
        let s = stem
            .to_str()
            .ok_or(ChangesFileNameParseError::InvalidName)?;
        let mut parts = s.split('_');
        let package = parts
            .next()
            .ok_or(ChangesFileNameParseError::MissingVersion)?;
        let version = parts
            .next()
            .ok_or(ChangesFileNameParseError::MissingVersion)?;
        let version = PackageVersion::parse(version)?;
        let architecture = parts
            .next()
            .ok_or(ChangesFileNameParseError::MissingArchitecture)?;
        let type_ = path
            .extension()
            .ok_or(ChangesFileNameParseError::InvalidName)?;
        let type_ = type_
            .to_str()
            .ok_or(ChangesFileNameParseError::InvalidName)?;

        Ok(ChangesFileInfo {
            package,
            version,
            architecture,
            type_,
        })
    }

    pub fn aptly_hash(&self) -> String {
        AptlyHashBuilder::default()
            .file(&AptlyHashFile {
                basename: &self.name,
                size: self.size,
                md5: &self.md5,
                sha1: &self.sha1,
                sha256: &self.sha256,
            })
            .finish()
    }
}

#[derive(Debug, Default)]
struct FileData {
    size: u64,
    md5: Option<String>,
    sha1: Option<String>,
    sha256: Option<String>,
}

fn changes_files_line(line: &str) -> Result<(String, u64, String), ChangesError> {
    let mut parts = line.split_ascii_whitespace();

    let digest = parts.next().ok_or(ChangesError::FilesParseError)?;
    let size: u64 = parts
        .next()
        .ok_or(ChangesError::FilesParseError)?
        .parse()
        .map_err(|_| ChangesError::FilesParseError)?;
    let _section = parts.next().ok_or(ChangesError::FilesParseError)?;
    let _priority = parts.next().ok_or(ChangesError::FilesParseError)?;
    let filename = parts.next().ok_or(ChangesError::FilesParseError)?;

    Ok((filename.to_string(), size, digest.to_string()))
}

fn changes_checksums_line(line: &str) -> Result<(String, u64, String), ChangesError> {
    let mut parts = line.split_ascii_whitespace();

    let digest = parts.next().ok_or(ChangesError::ChecksumsParseError)?;
    let size: u64 = parts
        .next()
        .ok_or(ChangesError::ChecksumsParseError)?
        .parse()
        .map_err(|_| ChangesError::ChecksumsParseError)?;
    let filename = parts.next().ok_or(ChangesError::ChecksumsParseError)?;

    Ok((filename.to_string(), size, digest.to_string()))
}

#[derive(thiserror::Error, Debug)]
pub enum ChangesFileToAptlyKeyError {
    #[error("Not a package type known to aptly")]
    UnsupportPackageType,
    #[error("Invalid package name in info")]
    InvalidPackageFile(#[from] ChangesFileNameParseError),
}

/*
impl TryFrom<&ChangesFile<'_>> for AptlyKey {
    type Error = ChangesFileToAptlyKeyError;

    fn try_from(c: &ChangesFile) -> Result<Self, Self::Error> {
        let info = c.parse_name()?;
        if info.type_ != "deb" && info.type_ != "udeb" {
            return Err(ChangesFileToAptlyKeyError::UnsupportPackageType);
        }

        let mut hasher = fnv::FnvHasher::default();

        hasher.write(c.name.as_bytes());
        hasher.write(&c.size.to_be_bytes());
        hasher.write(c.md5.as_bytes());
        hasher.write(c.sha1.as_bytes());
        hasher.write(c.sha256.as_bytes());

        let hash = format!("{:x}", hasher.finish());

        Ok(AptlyKey::new(
            info.architecture.to_string(),
            info.package.to_string(),
            c.changes.version().unwrap(),
            hash,
        ))
    }
}
*/
