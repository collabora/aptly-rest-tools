use color_eyre::{
    eyre::{bail, ensure, eyre},
    Result,
};
use debian_packaging::{
    deb::reader::{BinaryPackageEntry, BinaryPackageReader, ControlTarFile},
    package_version::PackageVersion,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs::File;
use tracing::{debug, error, info, warn};

use aptly_rest::{
    api::{files::UploadFiles, packages},
    dsc::DscFile,
    key::AptlyKey,
    AptlyRest, AptlyRestError,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PackageName {
    name: Arc<String>,
}

impl PackageName {
    fn new(name: String) -> Self {
        Self {
            name: Arc::new(name),
        }
    }

    #[allow(dead_code)]
    fn name(&self) -> &str {
        self.name.as_ref()
    }
}

impl std::fmt::Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

impl From<String> for PackageName {
    fn from(s: String) -> Self {
        PackageName::new(s)
    }
}

impl From<&str> for PackageName {
    fn from(s: &str) -> Self {
        PackageName::new(s.to_string())
    }
}

#[derive(Clone, Debug, Default)]
pub struct AptlyPackage {
    keys: BTreeSet<AptlyKey>,
}

impl AptlyPackage {
    pub fn new() -> Self {
        AptlyPackage::default()
    }

    pub fn push(&mut self, key: AptlyKey) {
        self.keys.insert(key);
    }

    pub fn keys(&self) -> impl Iterator<Item = &AptlyKey> {
        self.keys.iter()
    }

    #[tracing::instrument]
    pub fn newest(&self) -> Result<&AptlyKey> {
        self.keys()
            .reduce(|acc, key| {
                if key.version() > acc.version() {
                    key
                } else {
                    acc
                }
            })
            .ok_or_else(|| eyre!("Aptly package without keys"))
    }
}

#[derive(Clone, Debug)]
pub struct AptlyContent {
    repo: String,
    // Archicture -> packages -> aptlykey
    binary_arch: HashMap<String, BTreeMap<PackageName, AptlyPackage>>,
    // Package -> list of packages
    binary_indep: BTreeMap<PackageName, AptlyPackage>,
    // PackageName -> source
    sources: BTreeMap<PackageName, AptlyPackage>,
}

impl AptlyContent {
    #[tracing::instrument]
    pub async fn new_from_aptly(aptly: &AptlyRest, repo: String) -> Result<Self> {
        let packages = aptly.repo(&repo).packages().list().await?;
        let mut content = AptlyContent {
            repo,
            binary_arch: Default::default(),
            binary_indep: Default::default(),
            sources: Default::default(),
        };

        for p in packages {
            content.add_key(p);
        }
        Ok(content)
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn add_key(&mut self, key: AptlyKey) {
        if key.is_binary() {
            if key.arch() == "all" {
                self.binary_indep
                    .entry(key.package().into())
                    .or_default()
                    .push(key);
            } else {
                let map = match self.binary_arch.get_mut(key.arch()) {
                    Some(v) => v,
                    None => self.binary_arch.entry(key.arch().to_string()).or_default(),
                };
                map.entry(key.package().into()).or_default().push(key);
            }
        } else {
            self.sources
                .entry(key.package().into())
                .or_default()
                .push(key);
        }
    }
}

#[derive(Debug)]
pub struct OriginDeb {
    pub package: PackageName,
    pub architecture: String,
    pub path: PathBuf,
    pub from_source: Option<(PackageName, PackageVersion)>,
    pub aptly_hash: String,
}

impl OriginDeb {
    #[tracing::instrument]
    pub fn version(&self) -> Result<PackageVersion> {
        let f = std::fs::File::open(&self.path)?;
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

        bail!("Version not found in {}", self.path.display());
    }
}

impl Display for OriginDeb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.version() {
            Ok(version) => f.write_fmt(format_args!("{} {}", self.path.display(), version)),
            _ => f.write_fmt(format_args!("{} ???", self.path.display())),
        }
    }
}

#[derive(Debug, Default)]
pub struct OriginPackage {
    debs: Vec<OriginDeb>,
}

impl OriginPackage {
    pub fn debs(&self) -> &[OriginDeb] {
        &self.debs
    }

    pub fn push(&mut self, deb: OriginDeb) {
        self.debs.push(deb)
    }

    #[tracing::instrument]
    pub fn newest(&self) -> Result<&OriginDeb> {
        let mut n = self
            .debs
            .get(0)
            .ok_or_else(|| eyre!("No debs in package"))?;
        for deb in &self.debs[1..] {
            if deb.version()? > n.version()? {
                n = deb;
            }
        }

        Ok(n)
    }
}

#[derive(Debug)]
pub struct OriginDsc {
    pub package: PackageName,
    pub dsc_path: PathBuf,
    pub files: Vec<DscFile>,
    pub aptly_hash: String,
}

#[derive(Debug, Default)]
pub struct OriginSource {
    sources: Vec<OriginDsc>,
}

impl OriginSource {
    pub fn push(&mut self, dsc: OriginDsc) {
        self.sources.push(dsc)
    }

    pub fn sources(&self) -> &[OriginDsc] {
        &self.sources
    }
}

#[derive(Default)]
pub struct OriginContent {
    // architecture => { package name => binary packages }
    binary_arch: HashMap<String, BTreeMap<PackageName, OriginPackage>>,
    // package name => binary package
    binary_indep: BTreeMap<PackageName, OriginPackage>,
    // package name -> source package
    sources: BTreeMap<PackageName, OriginSource>,
}

#[derive(Default)]
pub struct OriginContentBuilder {
    content: OriginContent,
}

impl OriginContentBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    #[tracing::instrument(skip_all)]
    pub fn add_dsc(&mut self, dsc: OriginDsc) {
        self.content
            .sources
            .entry(dsc.package.clone())
            .or_default()
            .push(dsc);
    }

    #[tracing::instrument(skip_all)]
    pub fn add_deb(&mut self, deb: OriginDeb) {
        let dest = if deb.architecture == "all" {
            &mut self.content.binary_indep
        } else {
            self.content
                .binary_arch
                .entry(deb.architecture.to_string())
                .or_default()
        };

        dest.entry(deb.package.clone()).or_default().push(deb);
    }

    pub fn build(self) -> OriginContent {
        self.content
    }
}

#[async_trait::async_trait]
pub trait Syncer: Send {
    type Origin;
    // Packages only in the origin
    async fn add(
        &self,
        name: &PackageName,
        origin: &Self::Origin,
        actions: &mut SyncActions,
    ) -> Result<()>;
    //  Sync between the origin and aptly
    async fn sync(
        &self,
        name: &PackageName,
        origin: &Self::Origin,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()>;
}

#[derive(
    Debug, Default, Clone, Copy, PartialOrd, Ord, Hash, Eq, PartialEq, Serialize, Deserialize,
)]
pub enum MatchPoolPackageBy {
    #[default]
    KeyOnly,
    KeyOrFilename,
}

#[derive(Debug, Clone, PartialOrd, Ord, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum SyncAction {
    AddDeb {
        package: String,
        aptly_hash: String,
        path: PathBuf,
        match_existing: MatchPoolPackageBy,
    },
    AddDsc {
        package: String,
        aptly_hash: String,
        dsc_path: PathBuf,
        referenced_paths: Vec<PathBuf>,
    },
    AddPoolPackage(AptlyKey),
    RemoveAptly(AptlyKey),
}

#[derive(Default)]
struct PoolPackagesByName(HashMap<String, Vec<packages::Package>>);

impl PoolPackagesByName {
    #[tracing::instrument(skip(self))]
    fn find_matching_package(
        &self,
        package: &str,
        aptly_hash: &str,
        path: &Path,
        match_existing: MatchPoolPackageBy,
    ) -> Result<Option<AptlyKey>> {
        let Some(packages) = self.0.get(package) else {
            return Ok(None);
        };

        if let Some(existing_package_with_hash) =
            packages.iter().find(|m| m.key().hash() == aptly_hash)
        {
            return Ok(Some(existing_package_with_hash.key().clone()));
        }

        let filename = path.file_name().unwrap();
        if let Some(existing_package_with_filename) = packages.iter().find(|m| match *m {
            packages::Package::Source(source) => source
                .sha256_files()
                .iter()
                .any(|f| f.filename().ends_with(".dsc") && f.filename() == filename),
            packages::Package::Binary(binary) => binary.filename() == filename,
        }) {
            ensure!(
                match_existing == MatchPoolPackageBy::KeyOrFilename,
                "Package already exists with different key '{}'",
                existing_package_with_filename.key(),
            );
            return Ok(Some(existing_package_with_filename.key().clone()));
        }

        Ok(None)
    }
}

#[derive(Default)]
pub struct AddDebOptions {
    pub match_existing: MatchPoolPackageBy,
}

#[derive(Debug)]
pub struct SyncActions {
    aptly: AptlyRest,
    repo: String,
    actions: Vec<SyncAction>,
}

impl SyncActions {
    pub fn new(aptly: AptlyRest, repo: String) -> Self {
        Self {
            aptly,
            repo,
            actions: Vec::new(),
        }
    }

    pub fn add_deb(&mut self, d: &OriginDeb) {
        self.add_deb_with_options(d, &Default::default());
    }

    pub fn add_deb_with_options(&mut self, d: &OriginDeb, options: &AddDebOptions) {
        info!("Adding deb: {}", d.path.display());
        self.actions.push(SyncAction::AddDeb {
            package: d.package.name().to_owned(),
            aptly_hash: d.aptly_hash.clone(),
            path: d.path.clone(),
            match_existing: options.match_existing,
        });
    }

    #[tracing::instrument(skip_all)]
    pub fn add_dsc(&mut self, d: &OriginDsc) -> Result<()> {
        info!("Add dsc: {}", d.dsc_path.display());

        let (dsc_parent, dsc_filename) = match (d.dsc_path.parent(), d.dsc_path.file_name()) {
            (Some(parent), Some(filename)) => (parent, filename),
            _ => bail!("Invalid .dsc path '{}'", d.dsc_path.display()),
        };

        let referenced_paths = d
            .files
            .iter()
            // The .dsc references itself, so make sure we remove that
            // to avoid duplicates.
            .filter(|f| f.name.as_str() != dsc_filename)
            .map(|f| dsc_parent.join(&f.name))
            .collect();

        self.actions.push(SyncAction::AddDsc {
            package: d.package.name().to_owned(),
            aptly_hash: d.aptly_hash.clone(),
            dsc_path: d.dsc_path.to_owned(),
            referenced_paths,
        });
        Ok(())
    }

    pub fn remove_aptly(&mut self, k: AptlyKey) {
        info!("Remove from aptly: {k}");
        self.actions.push(SyncAction::RemoveAptly(k));
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    #[tracing::instrument(skip_all)]
    async fn query_pool_packages(&self) -> Result<PoolPackagesByName> {
        // Querying for all the packages results in a URL that is far too long
        // (over the 65k limit set by reqwest), so split it into 1k packages per
        // query.
        const CHUNK_SIZE: usize = 1000;

        let query_parts: Vec<_> = self
            .actions
            .iter()
            .filter_map::<&str, _>(|a| match a {
                SyncAction::AddDeb { package, .. } => Some(package),
                SyncAction::AddDsc { package, .. } => Some(package),
                _ => None,
            })
            .collect();

        let mut packages: Vec<packages::Package> = vec![];
        for chunk in query_parts.chunks(CHUNK_SIZE) {
            let query = chunk.to_vec().join("|");
            packages.extend(self.aptly.packages().query(query, false).detailed().await?);
        }

        let mut result: PoolPackagesByName = Default::default();
        for package in packages {
            result
                .0
                .entry(package.package().to_owned())
                .or_default()
                .push(package);
        }

        Ok(result)
    }

    #[tracing::instrument(skip_all)]
    async fn reuse_existing_pool_packages(&mut self) -> Result<()> {
        let pool_packages = self.query_pool_packages().await?;

        for action in &mut self.actions {
            match action {
                SyncAction::AddDeb {
                    package,
                    aptly_hash,
                    path,
                    match_existing,
                } => {
                    if let Some(key) = pool_packages.find_matching_package(
                        package,
                        aptly_hash,
                        path,
                        *match_existing,
                    )? {
                        info!("Using package '{key}' for '{}'", path.display());
                        *action = SyncAction::AddPoolPackage(key);
                    }
                }
                SyncAction::AddDsc {
                    package,
                    aptly_hash,
                    dsc_path,
                    ..
                } => {
                    if let Some(key) = pool_packages.find_matching_package(
                        package,
                        aptly_hash,
                        dsc_path,
                        MatchPoolPackageBy::KeyOnly,
                    )? {
                        info!("Using package '{key}' for '{}'", dsc_path.display());
                        *action = SyncAction::AddPoolPackage(key);
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn upload_file(&self, directory: String, path: &Path) -> Result<()> {
        info!("Uploading {}", path.display());

        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.to_owned())
            .ok_or_else(|| eyre!("Invalid path"))?;

        let file = File::open(path).await?;
        self.aptly
            .files()
            .directory(directory)
            .upload(UploadFiles::new().file(filename, file))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn apply(&self, upload_dir: &str) -> Result<()> {
        if self.actions.is_empty() {
            info!("Nothing to do.");
            return Ok(());
        }

        if let Err(err) = self
            .aptly
            .files()
            .directory(upload_dir.to_owned())
            .delete()
            .await
        {
            if let AptlyRestError::Request(inner) = &err {
                if inner.status() != Some(http::StatusCode::NOT_FOUND) {
                    return Err(err.into());
                }
            }
        }

        let mut uploaded_packages = 0;
        let mut to_remove = HashSet::<AptlyKey>::new();
        let mut to_reuse = HashSet::<AptlyKey>::new();

        for action in &self.actions {
            match action {
                SyncAction::AddDeb { path, .. } => {
                    self.upload_file(upload_dir.to_owned(), path).await?;
                    uploaded_packages += 1;
                }
                SyncAction::AddDsc {
                    dsc_path,
                    referenced_paths,
                    ..
                } => {
                    for path in std::iter::once(dsc_path).chain(referenced_paths) {
                        self.upload_file(upload_dir.to_owned(), path).await?;
                    }

                    uploaded_packages += 1;
                }
                SyncAction::AddPoolPackage(key) => {
                    to_reuse.insert(key.clone());
                }
                SyncAction::RemoveAptly(key) => {
                    to_remove.insert(key.clone());
                }
            }
        }

        if !to_reuse.is_empty() {
            info!(
                "Adding {} package(s) from pool to repository...",
                to_reuse.len()
            );

            self.aptly
                .repo(&self.repo)
                .packages()
                .add(&to_reuse)
                .await?;
            info!("Complete.");
        }

        if uploaded_packages != 0 {
            info!(
                "Adding {} newly uploaded package(s) to repository...",
                uploaded_packages
            );

            let response = self
                .aptly
                .repo(&self.repo)
                .files()
                .add_directory(upload_dir, &Default::default())
                .await?;
            debug!(?response);

            let warnings = response.report().warnings();
            if !warnings.is_empty() {
                warn!("Received {} warning(s):", warnings.len());
                for warning in warnings {
                    warn!(?warning);
                }
            }

            if !response.failed_files.is_empty() {
                error!("{} file(s) failed.", response.failed_files.len());
                bail!("Upload failed");
            }

            info!("Complete.");
        }

        if !to_remove.is_empty() {
            info!("Deleting {} package(s) from repository...", to_remove.len());

            self.aptly
                .repo(&self.repo)
                .packages()
                .delete(&to_remove)
                .await?;

            info!("Deletion complete.");
        }

        Ok(())
    }
}

// Calculate operation need to sync the origin into aptly
async fn sync_packages<S, O>(
    origin_iter: &mut dyn Iterator<Item = (&PackageName, &O)>,
    aptly_iter: &mut dyn Iterator<Item = (&PackageName, &AptlyPackage)>,
    syncer: &mut S,
    actions: &mut SyncActions,
) -> Result<()>
where
    S: Syncer<Origin = O>,
{
    let mut origin_iter = origin_iter.peekable();
    let mut aptly_iter = aptly_iter.peekable();
    loop {
        let (o, o_v, a, a_v) = match (origin_iter.peek(), aptly_iter.peek()) {
            (Some((o, o_v)), Some((a, a_v))) => (o, o_v, a, a_v),
            (None, Some((_, a_v))) => {
                for k in a_v.keys().cloned() {
                    actions.remove_aptly(k)
                }
                aptly_iter.next();
                continue;
            }
            (Some((o, o_v)), None) => {
                syncer.add(o, o_v, actions).await?;
                origin_iter.next();
                continue;
            }
            (None, None) => break,
        };

        match o.cmp(a) {
            std::cmp::Ordering::Less => {
                // Package in origin but not in aptly
                debug!("+ {o} - {a}");
                syncer.add(o, o_v, actions).await?;
                origin_iter.next();
            }
            std::cmp::Ordering::Equal => {
                debug!("* {o} - {a}");
                syncer.sync(o, o_v, a_v, actions).await?;
                origin_iter.next();
                aptly_iter.next();
            }
            std::cmp::Ordering::Greater => {
                // Package in aptly but not in origin (anymore)
                info!("== No longer in origin: {a} ==");
                for key in a_v.keys().cloned() {
                    actions.remove_aptly(key)
                }
                aptly_iter.next();
            }
        }
    }
    Ok(())
}

pub struct Syncers<BinaryDepSyncer, BinaryInDepSyncer, SourceSyncer>
where
    BinaryDepSyncer: Syncer<Origin = OriginPackage>,
    BinaryInDepSyncer: Syncer<Origin = OriginPackage>,
    SourceSyncer: Syncer<Origin = OriginSource>,
{
    pub binary_dep: BinaryDepSyncer,
    pub binary_indep: BinaryInDepSyncer,
    pub source: SourceSyncer,
}

/// Calculate what needs to be done to sync from origin repos to aptly
#[tracing::instrument(skip_all)]
pub async fn sync<BinaryDepSyncer, BinaryInDepSyncer, SourceSyncer>(
    origin_content: OriginContent,
    aptly: AptlyRest,
    aptly_content: AptlyContent,
    syncers: &mut Syncers<BinaryDepSyncer, BinaryInDepSyncer, SourceSyncer>,
) -> Result<SyncActions>
where
    BinaryDepSyncer: Syncer<Origin = OriginPackage>,
    BinaryInDepSyncer: Syncer<Origin = OriginPackage>,
    SourceSyncer: Syncer<Origin = OriginSource>,
{
    let mut actions = SyncActions::new(aptly, aptly_content.repo().to_owned());
    let architectures: HashSet<_> = origin_content
        .binary_arch
        .keys()
        .chain(aptly_content.binary_arch.keys())
        .collect();

    for arch in architectures {
        let mut origin_iter: Box<dyn Iterator<Item = _>> =
            if let Some(o) = origin_content.binary_arch.get(arch) {
                Box::new(o.iter())
            } else {
                Box::new(std::iter::empty())
            };

        let mut aptly_iter: Box<dyn Iterator<Item = _>> =
            if let Some(a) = aptly_content.binary_arch.get(arch) {
                Box::new(a.iter()) as Box<dyn Iterator<Item = _>>
            } else {
                Box::new(std::iter::empty()) as _
            };

        info!(" == Syncing {arch} ==");
        sync_packages(
            &mut origin_iter,
            &mut aptly_iter,
            &mut syncers.binary_dep,
            &mut actions,
        )
        .await?;
    }

    info!(" == Syncing arch indep packages == ");
    sync_packages(
        &mut origin_content.binary_indep.iter(),
        &mut aptly_content.binary_indep.iter(),
        &mut syncers.binary_indep,
        &mut actions,
    )
    .await?;

    info!(" == Syncing sources == ");
    sync_packages(
        &mut origin_content.sources.iter(),
        &mut aptly_content.sources.iter(),
        &mut syncers.source,
        &mut actions,
    )
    .await?;

    info!(" == Looking for existing packages in the pool ==");
    actions.reuse_existing_pool_packages().await?;

    info!(" == Actions calculated == ");

    Ok(actions)
}