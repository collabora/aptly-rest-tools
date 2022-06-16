use std::{
    collections::BTreeMap,
    hash::Hasher,
    io::{BufRead, Cursor},
    path::{Path, PathBuf},
};

use debian_packaging::{
    debian_source_control::{DebianSourceControlFile, DebianSourceControlFileEntry},
    error::DebianError,
    package_version::PackageVersion,
    repository::release::ChecksumType,
};
use tokio::{fs::File, io::AsyncReadExt};

use crate::key::AptlyKey;

pub struct Dsc {
    dsc: DebianSourceControlFile<'static>,
    path: PathBuf,
    size: u64,
    md5: String,
    sha1: String,
    sha256: String,
}

#[derive(thiserror::Error, Debug)]
pub enum DscError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to parse: {0}")]
    Parse(#[from] DebianError),
    #[error("Missing checksum for some file in dsc")]
    MissingChecksum,
    #[error("Missing Sha1 checksums line")]
    MissingSha1Checksums,
    #[error("Missing Sha256 checksums line")]
    MissingSha256Checksums,
}

fn hex_digest<H: digest::Digest>(data: &[u8]) -> String {
    let digest = H::digest(data);
    base16ct::lower::encode_string(&digest)
}

impl Dsc {
    pub async fn from_file(path: PathBuf) -> Result<Self, DscError> {
        let mut file = File::open(&path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;

        let mut cursor = Cursor::new(&data);
        let mut line = String::new();
        cursor.read_line(&mut line)?;
        cursor.set_position(0);

        let dsc = if line.starts_with("-----BEGIN PGP SIGNED MESSAGE-----") {
            DebianSourceControlFile::from_armored_reader(Cursor::new(&data))?
        } else {
            DebianSourceControlFile::from_reader(Cursor::new(&data))?
        };

        let md5 = hex_digest::<md5::Md5>(&data);
        let sha1 = hex_digest::<sha1::Sha1>(&data);
        let sha256 = hex_digest::<sha2::Sha256>(&data);

        Ok(Self {
            path,
            dsc,
            size: data.len() as u64,
            md5,
            sha1,
            sha256,
        })
    }

    pub fn source(&self) -> Result<&str, DscError> {
        Ok(self.dsc.source()?)
    }

    pub fn version(&self) -> Result<PackageVersion, DscError> {
        Ok(self.dsc.version()?)
    }

    /// Get a reference to the dsc's dsc.
    pub fn dsc(&self) -> &DebianSourceControlFile<'static> {
        &self.dsc
    }

    /// Get a reference to the dsc's md5.
    pub fn md5(&self) -> &str {
        self.md5.as_ref()
    }

    /// Get a reference to the dsc's sha1.
    pub fn sha1(&self) -> &str {
        self.sha1.as_ref()
    }

    /// Get a reference to the dsc's sha256.
    pub fn sha256(&self) -> &str {
        self.sha256.as_ref()
    }

    /// Get a reference to the dsc's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn files(&self) -> Result<Vec<DscFile>, DscError> {
        let mut files = BTreeMap::new();

        let filename = self
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        files.entry(filename).or_insert_with(|| FileData {
            size: self.size,
            md5: Some(self.md5.clone()),
            sha1: Some(self.sha1.clone()),
            sha256: Some(self.sha256.clone()),
        });

        update_dsc_files(&mut files, &mut self.dsc.files()?)?;
        update_dsc_files(
            &mut files,
            &mut self
                .dsc
                .checksums_sha1()
                .ok_or(DscError::MissingSha1Checksums)?,
        )?;
        update_dsc_files(
            &mut files,
            &mut self
                .dsc
                .checksums_sha256()
                .ok_or(DscError::MissingSha256Checksums)?,
        )?;

        files
            .iter()
            .map(|(name, data)| {
                Ok(DscFile {
                    name: name.to_string(),
                    size: data.size,
                    md5: data
                        .md5
                        .as_deref()
                        .ok_or(DscError::MissingChecksum)?
                        .to_string(),
                    sha1: data
                        .sha1
                        .as_deref()
                        .ok_or(DscError::MissingChecksum)?
                        .to_string(),
                    sha256: data
                        .sha256
                        .as_deref()
                        .ok_or(DscError::MissingChecksum)?
                        .to_string(),
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DscFile {
    pub name: String,
    pub size: u64,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default)]
struct FileData {
    size: u64,
    md5: Option<String>,
    sha1: Option<String>,
    sha256: Option<String>,
}

fn update_dsc_files(
    files: &mut BTreeMap<String, FileData>,
    iter: &mut dyn Iterator<Item = Result<DebianSourceControlFileEntry<'_>, DebianError>>,
) -> Result<(), DebianError> {
    for file in iter {
        let file = file?;
        let entry = files
            .entry(file.filename.to_string())
            .or_insert_with(|| FileData {
                size: file.size,
                ..Default::default()
            });

        let digest = file.digest.digest_hex();

        match file.digest.checksum_type() {
            ChecksumType::Md5 => entry.md5 = Some(digest),
            ChecksumType::Sha1 => entry.sha1 = Some(digest),
            ChecksumType::Sha256 => entry.sha256 = Some(digest),
        }
    }

    Ok(())
}

impl TryFrom<&Dsc> for AptlyKey {
    type Error = DscError;

    fn try_from(dsc: &Dsc) -> Result<Self, Self::Error> {
        let mut hasher = fnv::FnvHasher::default();

        for file in dsc.files()? {
            hasher.write(file.name.as_bytes());
            hasher.write(&file.size.to_be_bytes());
            hasher.write(file.md5.as_bytes());
            hasher.write(file.sha1.as_bytes());
            hasher.write(file.sha256.as_bytes());
        }

        let hash = format!("{:x}", hasher.finish());

        Ok(AptlyKey::new(
            "source".to_string(),
            dsc.source()?.to_string(),
            dsc.version()?,
            hash,
        ))
    }
}
