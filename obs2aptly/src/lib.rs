use anyhow::{anyhow, bail, Result};
use debian_packaging::{
    deb::reader::{BinaryPackageEntry, BinaryPackageReader, ControlTarFile},
    package_version::PackageVersion,
};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Display,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing::{debug, info, warn};

use aptly_rest::{
    changes::Changes,
    dsc::Dsc,
    key::AptlyKey,
    utils::scanner::{self, Scanner},
    AptlyRest,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct PackageName {
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

    pub fn newest(&self) -> Result<&AptlyKey> {
        self.keys()
            .reduce(|acc, key| {
                if key.version() > acc.version() {
                    key
                } else {
                    acc
                }
            })
            .ok_or_else(|| anyhow!("Aptly package without keys"))
    }
}

#[derive(Clone, Debug, Default)]
pub struct AptlyContent {
    // Archicture -> packages -> aptlykey
    binary_arch: HashMap<String, BTreeMap<PackageName, AptlyPackage>>,
    // Package -> list of packages
    binary_indep: BTreeMap<PackageName, AptlyPackage>,
    // PackageName -> source
    sources: BTreeMap<PackageName, AptlyPackage>,
}

impl AptlyContent {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn new_from_aptly(aptly: &AptlyRest, repo: &str) -> Result<Self> {
        let packages = aptly.repo(repo).packages().list().await?;
        let mut content = AptlyContent::new();

        for p in packages {
            content.add_key(p);
        }
        Ok(content)
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
struct ObsDeb {
    package: PackageName,
    path: PathBuf,
    source: String,
    source_version: PackageVersion,
    // Version from the changes file; will not have the epoch
    changes_version: PackageVersion,
    aptly_hash: String,
}

impl ObsDeb {
    fn version(&self) -> Result<PackageVersion> {
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

        Err(anyhow!("Version not found in .deb"))
    }
}

impl Display for ObsDeb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.version() {
            Ok(version) => f.write_fmt(format_args!("{} {}", self.path.display(), version)),
            _ => f.write_fmt(format_args!("{} ???", self.path.display())),
        }
    }
}

#[derive(Debug)]
struct ObsPackage {
    package: PackageName,
    debs: Vec<ObsDeb>,
}

impl ObsPackage {
    fn new(package: PackageName) -> Self {
        Self {
            package,
            debs: Vec::new(),
        }
    }

    #[allow(dead_code)]
    fn package(&self) -> &PackageName {
        &self.package
    }

    fn push(&mut self, deb: ObsDeb) {
        self.debs.push(deb)
    }

    fn newest(&self) -> Result<&ObsDeb> {
        let mut n = self
            .debs
            .get(0)
            .ok_or_else(|| anyhow!("No debs in package"))?;
        for deb in &self.debs[1..] {
            if deb.version()? > n.version()? {
                n = deb;
            }
        }

        Ok(n)
    }
}

struct ObsDsc {
    package: PackageName,
    dsc: Dsc,
    aptly_hash: String,
}

#[derive(Default)]
struct ObsSource {
    sources: Vec<ObsDsc>,
}

impl ObsSource {
    fn push(&mut self, dsc: ObsDsc) {
        self.sources.push(dsc)
    }
}

#[derive(Default)]
pub struct ObsContent {
    // TODO assumed OBS will only have *one* .deb per package name
    // architecture => { package name => changesfiles }
    binary_arch: HashMap<String, BTreeMap<PackageName, ObsPackage>>,
    // package => [ keys ]
    // OBS build arch all for each architecture so typically so it's repositories end up having one
    // copy of the arch all package per architecture
    binary_indep: BTreeMap<PackageName, ObsPackage>,
    // PackageName -> Source
    sources: BTreeMap<PackageName, ObsSource>,
}

impl ObsContent {
    pub async fn new_from_path(path: PathBuf) -> Result<Self> {
        let mut content: Self = Default::default();
        let mut scanner = Scanner::new(path);

        while let Some(control) = scanner.try_next().await? {
            match control {
                scanner::Found::Changes(c) => content.add_changes(c)?,
                scanner::Found::Dsc(d) => content.add_dsc(d)?,
            }
        }
        Ok(content)
    }

    fn add_dsc(&mut self, dsc: Dsc) -> Result<()> {
        let a: AptlyKey = (&dsc).try_into()?;
        let package: PackageName = a.package().into();
        let dsc = ObsDsc {
            package: package.clone(),
            dsc,
            aptly_hash: a.hash().to_string(),
        };
        self.sources
            .entry(a.package().into())
            .or_default()
            .push(dsc);
        Ok(())
    }

    fn add_changes(&mut self, changes: Changes) -> Result<()> {
        for f in changes.files()? {
            if !f.name.ends_with(".deb") && !f.name.ends_with(".udeb") {
                continue;
            }
            let info = f.parse_name()?;
            let package_name: PackageName = info.package.into();
            let deb = ObsDeb {
                package: package_name.clone(),
                path: changes.path().clone().with_file_name(&f.name),
                source: changes.source()?.to_string(),
                source_version: changes.version()?,
                changes_version: info.version,
                aptly_hash: f.aptly_hash(),
            };
            if info.architecture == "all" {
                self.binary_indep
                    .entry(package_name.clone())
                    .or_insert_with(|| ObsPackage::new(package_name))
                    .push(deb);
            } else {
                self.binary_arch
                    .entry(info.architecture.to_string())
                    .or_default()
                    .entry(package_name.clone())
                    .or_insert_with(|| ObsPackage::new(package_name))
                    .push(deb);
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
trait Syncer: Send {
    type Obs;
    // Packages only in OBS
    async fn add(&self, obs: &Self::Obs, actions: &mut SyncActions) -> Result<()>;
    //  Sync between obs and aplty
    async fn sync(
        &self,
        obs: &Self::Obs,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()>;
}

struct BinaryDepSyncer {}

#[async_trait::async_trait]
impl Syncer for BinaryDepSyncer {
    type Obs = ObsPackage;

    async fn add(&self, obs: &Self::Obs, actions: &mut SyncActions) -> Result<()> {
        let obs_newest = obs.newest()?;
        actions.add_deb(obs_newest);
        Ok(())
    }

    async fn sync(
        &self,
        obs: &Self::Obs,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // Simple case just one package on both sides
        let obs_newest = obs.newest()?;
        let aptly_newest = aptly.newest()?;
        if &obs_newest.version()? < aptly_newest.version() {
            warn!("{} older then {} in aptly", obs_newest, aptly_newest);
        } else if obs_newest.aptly_hash != aptly_newest.hash() {
            info!("== Changes for {} ==", obs.package);
            for key in aptly.keys().cloned() {
                actions.remove_aptly(key);
            }
            actions.add_deb(obs_newest);
        }
        Ok(())
    }
}

struct BinaryInDepSyncer {}
#[async_trait::async_trait]
impl Syncer for BinaryInDepSyncer {
    type Obs = ObsPackage;
    async fn add(&self, obs: &Self::Obs, actions: &mut SyncActions) -> Result<()> {
        // Only add the newest arch all package; Potential future update could be to add one deb
        // for each *version*
        let obs_newest = obs.newest()?;
        actions.add_deb(obs_newest);
        Ok(())
    }

    async fn sync(
        &self,
        obs: &Self::Obs,
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
        //   if match not found check if changes_version is *newer* then all aptly version; if
        //   so add it
        //      -> common when doing a package update
        //
        //   if not fall back checking the exact version
        //   get the exact version of the .deb as it may have an epoch which means it's newer
        //     -> shortcut if aplty has no epoch and changes version matches?
        //
        //
        info!("=== Changes for {} ===", obs.package());
        let mut keep_in_aptly = Vec::new();

        #[derive(Debug, Hash, PartialEq, Eq)]
        struct BySourceVersion<'a> {
            source: &'a str,
            version: &'a PackageVersion,
        }
        let obs_by_version: HashMap<BySourceVersion, Vec<&ObsDeb>> =
            obs.debs.iter().fold(HashMap::new(), |mut acc, d| {
                let t = BySourceVersion {
                    source: &d.source,
                    version: &d.source_version,
                };
                acc.entry(t).or_default().push(&d);
                acc
            });

        for (source, v) in obs_by_version.iter() {
            info!(
                "=== Changes for source {} - {} ===",
                source.source, source.version
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
            let version = &v[0].version()?;
            // If version is newer then everything in aptly add the package
            // TODO actual package version
            if aptly.keys().all(|a| a.version() < version) {
                actions.add_deb(v[0]);
                continue;
            }

            // If any of the aptly keys match the exact package version, then happyness?
            if let Some(found) = aptly.keys().find(|a| a.version() == version) {
                info!("Keeping {} as it matches a version in OBS", found);
                keep_in_aptly.push(found);
            }
        }

        // TODO: Don't forget to remove old arch all...
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

struct SourceSyncer {}

#[async_trait::async_trait]
impl Syncer for SourceSyncer {
    type Obs = ObsSource;

    async fn add(&self, obs: &Self::Obs, actions: &mut SyncActions) -> Result<()> {
        for source in &obs.sources {
            actions.add_dsc(source.dsc.path().to_path_buf());
        }

        Ok(())
    }

    async fn sync(
        &self,
        obs: &Self::Obs,
        aptly: &AptlyPackage,
        actions: &mut SyncActions,
    ) -> Result<()> {
        // For source for a given package name only expect one from OBS and one from aptly
        // TODO let aptly keep all source version referred to by changes files? Though this would
        // need to account for build suffixes in some way
        // Simple case just one package on both sides
        if obs.sources.len() == 1 {
            let d = &obs.sources[0];
            let a = aptly.keys().next().unwrap();

            if d.aptly_hash != a.hash() {
                // TODO make sure version is upgraded
                actions.remove_aptly(a.clone());
                actions.add_dsc(d.dsc.path().to_path_buf());
            }
        } else {
            todo!("unimplemented");
        }

        Ok(())
    }
}

// Calculate operation need to sync obs into aptly
async fn sync_packages<S, O>(
    obs_iter: &mut dyn Iterator<Item = (&PackageName, &O)>,
    aptly_iter: &mut dyn Iterator<Item = (&PackageName, &AptlyPackage)>,
    syncer: &mut S,
    actions: &mut SyncActions,
) -> Result<()>
where
    S: Syncer<Obs = O>,
{
    let mut obs_iter = obs_iter.peekable();
    let mut aptly_iter = aptly_iter.peekable();
    loop {
        let (o, o_v, a, a_v) = match (obs_iter.peek(), aptly_iter.peek()) {
            (Some((o, o_v)), Some((a, a_v))) => (o, o_v, a, a_v),
            (None, Some((_, a_v))) => {
                for k in a_v.keys().cloned() {
                    actions.remove_aptly(k)
                }
                aptly_iter.next();
                continue;
            }
            (Some((_, o_v)), None) => {
                syncer.add(o_v, actions).await?;
                obs_iter.next();
                continue;
            }
            (None, None) => break,
        };

        if o < a {
            // Package in obs but not in aptly
            debug!("+ {o} - {a}");
            syncer.add(o_v, actions).await?;
            obs_iter.next();
        } else if o == a {
            debug!("* {o} - {a}");
            syncer.sync(o_v, a_v, actions).await?;
            obs_iter.next();
            aptly_iter.next();
        } else {
            // Package in aptly but not in obs (anymore)
            info!("== No longer in OBS:  {a} ==");
            for key in a_v.keys().cloned() {
                actions.remove_aptly(key)
            }
            aptly_iter.next();
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialOrd, Ord, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum SyncAction {
    AddDeb(PathBuf),
    AddDsc(PathBuf),
    RemoveAptly(AptlyKey),
}

#[derive(Debug)]
pub struct SyncActions {
    aptly: AptlyRest,
    actions: Vec<SyncAction>,
}

impl SyncActions {
    fn new(aptly: AptlyRest) -> Self {
        Self {
            aptly,
            actions: Vec::new(),
        }
    }

    fn add_deb(&mut self, d: &ObsDeb) {
        info!("Adding deb: {}", d.path.display());
        self.actions.push(SyncAction::AddDeb(d.path.clone()));
    }

    fn add_dsc(&mut self, d: PathBuf) {
        info!("Add dsc: {}", d.display());
        self.actions.push(SyncAction::AddDsc(d));
    }

    fn remove_aptly(&mut self, k: AptlyKey) {
        info!("Remove from aptly: {k}");
        self.actions.push(SyncAction::RemoveAptly(k));
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }
}

/// Calculate what needs to be done to sync from obs repos to aptly
pub async fn sync(
    aptly: AptlyRest,
    obs_content: ObsContent,
    aptly_content: AptlyContent,
) -> Result<SyncActions> {
    let mut actions = SyncActions::new(aptly);
    let architectures: HashSet<_> = obs_content
        .binary_arch
        .keys()
        .chain(aptly_content.binary_arch.keys())
        .collect();

    for arch in architectures {
        let mut syncer = BinaryDepSyncer {};

        let mut obs_iter: Box<dyn Iterator<Item = _>> =
            if let Some(o) = obs_content.binary_arch.get(arch) {
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
        sync_packages(&mut obs_iter, &mut aptly_iter, &mut syncer, &mut actions).await?;
    }

    info!(" == Syncing arch indep packages == ");
    let mut syncer = BinaryInDepSyncer {};
    sync_packages(
        &mut obs_content.binary_indep.iter(),
        &mut aptly_content.binary_indep.iter(),
        &mut syncer,
        &mut actions,
    )
    .await?;

    info!(" == Ssyncing sources == ");
    let mut syncer = SourceSyncer {};
    sync_packages(
        &mut obs_content.sources.iter(),
        &mut aptly_content.sources.iter(),
        &mut syncer,
        &mut actions,
    )
    .await?;
    info!(" == Actions calculated == ");

    Ok(actions)
}
