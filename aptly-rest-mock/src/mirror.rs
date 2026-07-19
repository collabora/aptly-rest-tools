use std::collections::HashMap;

use crate::MirrorData;

#[derive(Debug, Clone)]
pub struct Mirrors {
    mirrors: HashMap<String, Mirror>,
}

impl Default for Mirrors {
    fn default() -> Self {
        Self::new()
    }
}

impl Mirrors {
    pub fn new() -> Self {
        Self {
            mirrors: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.mirrors.len()
    }

    pub fn get(&self, name: &str) -> Option<&Mirror> {
        self.mirrors.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Mirror> {
        self.mirrors.get_mut(name)
    }

    pub fn add(&mut self, mirror: Mirror) {
        self.mirrors.insert(mirror.data.name.clone(), mirror);
    }

    pub fn add_package(&mut self, mirror: &str, key: String) {
        let mirror = self.mirrors.get_mut(mirror).expect("Mirror not known");
        mirror.add_package(key);
    }
}

impl<'a> IntoIterator for &'a Mirrors {
    type Item = &'a Mirror;
    type IntoIter = MirrorsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let values = self.mirrors.values();
        MirrorsIter { values }
    }
}

pub struct MirrorsIter<'a> {
    values: std::collections::hash_map::Values<'a, String, Mirror>,
}

impl<'a> Iterator for MirrorsIter<'a> {
    type Item = &'a Mirror;

    fn next(&mut self) -> Option<Self::Item> {
        self.values.next()
    }
}

impl From<MirrorData> for Mirror {
    fn from(data: MirrorData) -> Self {
        Mirror {
            data,
            packages: Vec::new(),
            applied_update: None,
        }
    }
}

/// The parameters of the most recent update (PUT) request for a mirror,
/// recorded by the mock so tests can verify what the client sent.
#[derive(Clone, Debug, Default)]
pub struct AppliedUpdate {
    pub rename: Option<String>,
    pub keyrings: Vec<String>,
    pub ignore_checksums: bool,
    pub ignore_signatures: bool,
    pub force_update: bool,
    pub skip_existing_packages: bool,
    pub latest_only: bool,
}

#[derive(Clone, Debug)]
pub struct Mirror {
    pub(crate) data: MirrorData,
    packages: Vec<String>,
    applied_update: Option<AppliedUpdate>,
}

impl Mirror {
    pub fn name(&self) -> &str {
        &self.data.name
    }

    pub fn uuid(&self) -> &str {
        &self.data.uuid
    }

    pub fn distribution(&self) -> &str {
        &self.data.distribution
    }

    pub fn archive_root(&self) -> &str {
        &self.data.archive_root
    }

    pub fn components(&self) -> &[String] {
        self.data.components.as_slice()
    }

    pub fn architectures(&self) -> &[String] {
        self.data.architectures.as_slice()
    }

    pub fn filter(&self) -> &str {
        &self.data.filter
    }

    pub fn keyrings(&self) -> &[String] {
        self.data.keyrings.as_slice()
    }

    pub fn download_sources(&self) -> bool {
        self.data.download_sources
    }

    pub fn download_udebs(&self) -> bool {
        self.data.download_udebs
    }

    pub fn download_installer(&self) -> bool {
        self.data.download_installer
    }

    pub fn download_app_stream(&self) -> bool {
        self.data.download_app_stream
    }

    pub fn filter_with_deps(&self) -> bool {
        self.data.filter_with_deps
    }

    pub fn skip_component_check(&self) -> bool {
        self.data.skip_component_check
    }

    pub fn skip_architecture_check(&self) -> bool {
        self.data.skip_architecture_check
    }

    pub fn ignore_signatures(&self) -> bool {
        self.data.ignore_signatures
    }

    pub(crate) fn add_package(&mut self, package: String) {
        self.packages.push(package)
    }

    pub fn packages(&self) -> &[String] {
        &self.packages
    }

    pub(crate) fn set_applied_update(&mut self, update: AppliedUpdate) {
        self.applied_update = Some(update);
    }

    pub fn applied_update(&self) -> Option<&AppliedUpdate> {
        self.applied_update.as_ref()
    }
}
