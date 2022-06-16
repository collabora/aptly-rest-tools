use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Repositories {
    repositories: HashMap<String, Repository>,
}

impl Default for Repositories {
    fn default() -> Self {
        Self::new()
    }
}

impl Repositories {
    pub fn new() -> Self {
        Self {
            repositories: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.repositories.len()
    }

    pub fn get(&self, name: &str) -> Option<&Repository> {
        self.repositories.get(name)
    }

    pub fn add(&mut self, name: String, comment: String, distribution: String, component: String) {
        let r = Repository::new(name.clone(), comment, distribution, component);
        self.repositories.insert(name, r);
    }

    pub fn add_package(&mut self, repo: &str, key: String) {
        let repo = self
            .repositories
            .get_mut(repo)
            .expect("Repository not known");
        repo.add_package(key);
    }
}

impl<'a> IntoIterator for &'a Repositories {
    type Item = &'a Repository;
    type IntoIter = RepositoriesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let values = self.repositories.values();
        RepositoriesIter { values }
    }
}

pub struct RepositoriesIter<'a> {
    values: std::collections::hash_map::Values<'a, String, Repository>,
}

impl<'a> Iterator for RepositoriesIter<'a> {
    type Item = &'a Repository;

    fn next(&mut self) -> Option<Self::Item> {
        self.values.next()
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    pub name: String,
    pub comment: String,
    pub distribution: String,
    pub component: String,
    packages: Vec<String>,
}

impl Repository {
    fn new(name: String, comment: String, distribution: String, component: String) -> Self {
        Self {
            name,
            comment,
            distribution,
            component,
            packages: Vec::new(),
        }
    }

    pub(crate) fn add_package(&mut self, package: String) {
        self.packages.push(package)
    }

    pub fn packages(&self) -> &[String] {
        &self.packages
    }
}
