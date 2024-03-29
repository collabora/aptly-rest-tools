use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, NoneAsEmptyString};

use crate::{key::AptlyKey, AptlyRestError};

#[derive(Debug, Clone)]
pub struct RepoApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) name: String,
}

impl RepoApi<'_> {
    pub fn packages(&self) -> RepoApiPackages {
        RepoApiPackages { repo: self }
    }

    pub fn files(&self) -> RepoApiFiles {
        RepoApiFiles { repo: self }
    }

    pub async fn get(&self) -> Result<Repo, AptlyRestError> {
        self.aptly
            .get(self.aptly.url(&["api", "repos", &self.name]))
            .await
    }

    pub async fn snapshot(
        &self,
        name: &str,
        options: &SnapshotOptions,
    ) -> Result<crate::Snapshot, AptlyRestError> {
        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct SnapshotRequest<'a> {
            name: &'a str,
            #[serde(flatten)]
            options: &'a SnapshotOptions,
        }

        self.aptly
            .post_body(
                self.aptly.url(&["api", "repos", &self.name, "snapshots"]),
                &SnapshotRequest { name, options },
            )
            .await
    }

    pub async fn delete(&self, options: &DeleteOptions) -> Result<(), AptlyRestError> {
        let mut url = self.aptly.url(&["api", "repos", &self.name]);

        {
            let mut pairs = url.query_pairs_mut();
            if options.force {
                pairs.append_pair("force", "1");
            }
        }

        self.aptly
            .send_request(self.aptly.client.delete(url))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RepoApiPackages<'a> {
    repo: &'a crate::RepoApi<'a>,
}

impl RepoApiPackages<'_> {
    fn base_url(&self) -> Url {
        self.repo
            .aptly
            .url(&["api", "repos", &self.repo.name, "packages"])
    }

    fn search_url(&self, query: Option<&str>, with_deps: bool, detailed: bool) -> Url {
        let mut url = self.base_url();

        let mut pairs = url.query_pairs_mut();
        if let Some(query) = query {
            pairs.append_pair("q", query);
            if with_deps {
                pairs.append_pair("withDeps", "1");
            }
        }

        if detailed {
            pairs.append_pair("format", "details");
        }

        drop(pairs);
        url
    }

    async fn do_list(
        &self,
        query: Option<&str>,
        with_deps: bool,
    ) -> Result<Vec<AptlyKey>, AptlyRestError> {
        let url = self.search_url(query, with_deps, false);
        self.repo.aptly.get(url).await
    }

    async fn do_detailed(
        &self,
        query: Option<&str>,
        with_deps: bool,
    ) -> Result<Vec<Package>, AptlyRestError> {
        let url = self.search_url(query, with_deps, true);
        self.repo.aptly.get(url).await
    }

    pub async fn list(&self) -> Result<Vec<AptlyKey>, AptlyRestError> {
        self.do_list(None, false).await
    }

    pub async fn detailed(&self) -> Result<Vec<Package>, AptlyRestError> {
        self.do_detailed(None, false).await
    }

    pub fn query(&self, query: String, with_deps: bool) -> RepoApiPackagesQuery {
        RepoApiPackagesQuery {
            parent: self,
            query,
            with_deps,
        }
    }

    pub async fn add<'r, R>(&self, keys: R) -> Result<Repo, AptlyRestError>
    where
        R: IntoIterator<Item = &'r AptlyKey>,
    {
        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct AddRequest<'r> {
            package_refs: Vec<&'r AptlyKey>,
        }

        self.repo
            .aptly
            .post_body(
                self.base_url(),
                &AddRequest {
                    package_refs: keys.into_iter().collect(),
                },
            )
            .await
    }

    pub async fn delete<'r, R>(&self, keys: R) -> Result<(), AptlyRestError>
    where
        R: IntoIterator<Item = &'r AptlyKey>,
    {
        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct DeleteRequest<'r> {
            package_refs: Vec<&'r AptlyKey>,
        }

        let req = self
            .repo
            .aptly
            .client
            .delete(self.base_url())
            .json(&DeleteRequest {
                package_refs: keys.into_iter().collect(),
            });
        self.repo.aptly.send_request(req).await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RepoApiPackagesQuery<'a> {
    parent: &'a RepoApiPackages<'a>,
    query: String,
    with_deps: bool,
}

impl RepoApiPackagesQuery<'_> {
    pub async fn list(&self) -> Result<Vec<AptlyKey>, AptlyRestError> {
        self.parent.do_list(Some(&self.query), self.with_deps).await
    }

    pub async fn detailed(&self) -> Result<Vec<Package>, AptlyRestError> {
        self.parent
            .do_detailed(Some(&self.query), self.with_deps)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct RepoApiFiles<'a> {
    repo: &'a crate::RepoApi<'a>,
}

impl RepoApiFiles<'_> {
    fn url(&self, directory: &str, filename: Option<&str>, options: &AddPackageOptions) -> Url {
        let mut path = vec!["api", "repos", &self.repo.name, "file", directory];
        if let Some(filename) = filename {
            path.push(filename);
        }

        let mut url = self.repo.aptly.url(path);

        let mut pairs = url.query_pairs_mut();
        if options.force_replace {
            pairs.append_pair("forceReplace", "1");
        }
        if options.no_remove {
            pairs.append_pair("noRemove", "1");
        }

        drop(pairs);
        url
    }

    pub async fn add_directory(
        &self,
        directory: &str,
        options: &AddPackageOptions,
    ) -> Result<AddPackageResponse, AptlyRestError> {
        self.repo
            .aptly
            .post(self.url(directory, None, options))
            .await
    }

    pub async fn add_file(
        &self,
        directory: &str,
        filename: &str,
        options: &AddPackageOptions,
    ) -> Result<AddPackageResponse, AptlyRestError> {
        self.repo
            .aptly
            .post(self.url(directory, Some(filename), options))
            .await
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Repo {
    name: String,
    #[serde_as(as = "NoneAsEmptyString")]
    comment: Option<String>,
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "DefaultDistribution")]
    distribution: Option<String>,
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "DefaultComponent")]
    component: Option<String>,
}

impl Repo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            comment: None,
            distribution: None,
            component: None,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    pub fn with_comment(self, comment: Option<String>) -> Self {
        Self { comment, ..self }
    }

    pub fn distribution(&self) -> Option<&str> {
        self.distribution.as_deref()
    }

    pub fn with_distribution(self, distribution: Option<String>) -> Self {
        Self {
            distribution,
            ..self
        }
    }

    pub fn component(&self) -> Option<&str> {
        self.component.as_deref()
    }

    pub fn with_component(self, component: Option<String>) -> Self {
        Self { component, ..self }
    }
}

#[derive(Default, Debug)]
pub struct AddPackageOptions {
    pub no_remove: bool,
    pub force_replace: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AddPackageResponse {
    pub failed_files: Vec<String>,
    pub report: OperationReport,
}

impl AddPackageResponse {
    pub fn failed_files(&self) -> &[String] {
        &self.failed_files
    }

    pub fn report(&self) -> &OperationReport {
        &self.report
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct OperationReport {
    warnings: Vec<String>,
    added: Vec<String>,
    removed: Vec<String>,
}

impl OperationReport {
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn added(&self) -> &[String] {
        &self.added
    }

    pub fn removed(&self) -> &[String] {
        &self.removed
    }
}

#[derive(Default, Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SnapshotOptions {
    pub description: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct DeleteOptions {
    pub force: bool,
}

#[serde_as]
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Source {
    package: String,
    version: String,
    #[serde_as(as = "DisplayFromStr")]
    key: AptlyKey,
    architecture: String,
    #[serde(rename = "Checksums-Sha256")]
    sha256: String,
    #[serde(flatten)]
    _unparsed: serde_json::Value,
}

impl Source {
    pub fn package(&self) -> &str {
        self.package.as_ref()
    }

    pub fn version(&self) -> &str {
        self.version.as_ref()
    }

    pub fn key(&self) -> &AptlyKey {
        &self.key
    }

    pub fn architecture(&self) -> &str {
        self.architecture.as_ref()
    }

    pub fn sha256(&self) -> &str {
        self.sha256.as_ref()
    }
}

#[serde_as]
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Binary {
    package: String,
    version: String,
    architecture: String,
    #[serde_as(as = "DisplayFromStr")]
    key: AptlyKey,
    #[serde(rename = "SHA256")]
    sha256: String,
    #[serde(flatten)]
    _unparsed: serde_json::Value,
}

impl Binary {
    pub fn package(&self) -> &str {
        self.package.as_ref()
    }

    pub fn version(&self) -> &str {
        self.version.as_ref()
    }

    pub fn architecture(&self) -> &str {
        self.architecture.as_ref()
    }

    pub fn key(&self) -> &AptlyKey {
        &self.key
    }

    pub fn sha256(&self) -> &str {
        self.sha256.as_ref()
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Package {
    Binary(Binary),
    Source(Source),
}

impl Package {
    pub fn package(&self) -> &str {
        match self {
            Package::Binary(b) => &b.package,
            Package::Source(s) => &s.package,
        }
    }

    pub fn architecture(&self) -> &str {
        match self {
            Package::Binary(b) => &b.architecture,
            Package::Source(s) => &s.architecture,
        }
    }

    pub fn key(&self) -> &AptlyKey {
        match self {
            Package::Binary(b) => &b.key,
            Package::Source(s) => &s.key,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Package::Binary(b) => &b.version,
            Package::Source(s) => &s.version,
        }
    }

    pub fn sha256(&self) -> &str {
        match self {
            Package::Binary(b) => &b.sha256,
            Package::Source(s) => &s.sha256,
        }
    }

    pub fn is_source(&self) -> bool {
        matches!(self, Package::Source(_))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_binary() {
        let v: Package = serde_json::from_value(json!({
            "Architecture": "amd64",
  			"Breaks": "libstd-rust-dev (<< 1.25.0+dfsg1-2~~)",
            "Depends": "libc6 (>= 2.14), libgcc-s1 (>= 3.0), libstd-rust-dev (= 1.48.0+dfsg1-2), gcc, libc-dev, binutils (>= 2.26)",
            "Description": " Rust systems programming language\n",
            "Description-Md5": "67ca6080eea53dc7f3cdf73bc6b8521e",
            "Filename": "rustc_1.48.0+dfsg1-2_amd64.deb",
            "FilesHash": "87415bdc9ef60793",
            "Homepage": "http://www.rust-lang.org/",
            "Installed-Size": "5720",
            "Key": "Pamd64 rustc 1.48.0+dfsg1-2 87415bdc9ef60793",
            "MD5sum": "0302b014f85fc6a49418fae4ee34ea71",
            "Maintainer": "Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>",
            "Multi-Arch": "allowed",
            "Package": "rustc",
            "Priority": "optional",
            "Recommends": "cargo (>= 0.49.0~~), cargo (<< 0.50.0~~), rust-gdb | rust-lldb",
            "Replaces": "libstd-rust-dev (<< 1.25.0+dfsg1-2~~)",
            "SHA1": "a87f2104dfcc33fe204b6058826fcf10e8061118",
            "SHA256": "3cc857f1d9d5970d5d8ced830efb054520f820a4a496b6e55d68dbae19270137",
            "SHA512": "eece2581b0fb8804e360a70c4323ac4de78ed7a8753f8879bc84e0a540e23cbfb35e852abcc55500fd36ff491ae4fcb1f41c974e7e7ad8fba9b7bc453d769c68",
            "Section": "rust",
            "ShortKey": "Pamd64 rustc 1.48.0+dfsg1-2",
            "Size": "2049372",
            "Suggests": "rust-doc, rust-src, lld-11",
            "Version": "1.48.0+dfsg1-2"
        }))
        .unwrap();

        assert_eq!("rustc", v.package());
    }

    #[test]
    fn deserialize_source() {
        let v: Package = serde_json::from_value(json!({
            "Architecture": "any all",
            "Binary": "rustc, libstd-rust-1.48, libstd-rust-dev, libstd-rust-dev-windows, libstd-rust-dev-wasm32, rust-gdb, rust-lldb, rust-doc, rust-src",
            "Build-Conflicts": "gdb-minimal <!nocheck>",
            "Build-Depends": "debhelper (>= 9), debhelper-compat (= 12), dpkg-dev (>= 1.17.14), python3:native, cargo:native (>= 0.40.0) <!pkg.rustc.dlstage0>, rustc:native (>= 1.47.0+dfsg) <!pkg.rustc.dlstage0>, rustc:native (<= 1.48.0++) <!pkg.rustc.dlstage0>, llvm-11-dev:native, llvm-11-tools:native, gcc-mingw-w64-x86-64-posix:native [amd64] <!nowindows>, libllvm11, cmake (>= 3.0) | cmake3, pkg-config, zlib1g-dev:native, zlib1g-dev, liblzma-dev:native, binutils (>= 2.26) <!nocheck> | binutils-2.26 <!nocheck>, git <!nocheck>, procps <!nocheck>, gdb (>= 7.12) <!nocheck>, curl <pkg.rustc.dlstage0>, ca-certificates <pkg.rustc.dlstage0>",
            "Build-Depends-Indep": "wasi-libc (>= 0.0~git20200731.215adc8~~) <!nowasm>, wasi-libc (<= 0.0~git20200731.215adc8++) <!nowasm>, clang-11:native",
            "Checksums-Sha1": " 8de25025b60c8ddd82be84e1d75452209d8b17cd 76924 rustc_1.48.0+dfsg1-2.debian.tar.xz\n 54fb437e448ff407797d4a9270bd67664487e735 2665 rustc_1.48.0+dfsg1-2.dsc\n 7d2c6a2c01f86107eb1a40ecdbe59c79da2bbd79 22048320 rustc_1.48.0+dfsg1.orig.tar.xz\n",
            "Checksums-Sha256": " 7b4db2ce181dc3d8999388c7ea32ac1a992b699dbc70e4b6cd0b88831437c5ff 76924 rustc_1.48.0+dfsg1-2.debian.tar.xz\n 41994d5bd2b33e25b541b330173061e9748eca95144cb52074cd5c9277bb6468 2665 rustc_1.48.0+dfsg1-2.dsc\n f39dd5901feb713bc8876a042c3105bf654177878d8bcc71962c8dcc041af367 22048320 rustc_1.48.0+dfsg1.orig.tar.xz\n",
            "Checksums-Sha512": " a2457eb492cd57f4d15c8d6000099feba0d812337680578bc3d5e3ea2113df0b391d6d85b719fb87431b3b61ace2060e3ec7e89a59cd538b4a32f0889eac66c5 76924 rustc_1.48.0+dfsg1-2.debian.tar.xz\n 31beccc447e2cb5c583bfac7451e63f63b2b6eed4bb68e26e15de345bc17c735a250b9ac22b06867ce3354f9d85ecb480efc18c3fb4f685eaa84ee13edfe6566 2665 rustc_1.48.0+dfsg1-2.dsc\n ef98bae8efd8094d948b317f24dbe0a4905526e1e2d46c73054f37053784e0d6292dc36bd7d06485772f56a07f1d1bed2b84202ae7593486ea936a59a5e4f837 22048320 rustc_1.48.0+dfsg1.orig.tar.xz\n",
            "Directory": "pool/main/r/rustc",
            "Files": " bf0a264a093cc9ed2bcaa5b96d6de84d 76924 rustc_1.48.0+dfsg1-2.debian.tar.xz\n 5cae62a59034d342f2abcbb2ed4bce79 2665 rustc_1.48.0+dfsg1-2.dsc\n a429436119d1d92c53524836c3017f63 22048320 rustc_1.48.0+dfsg1.orig.tar.xz\n",
            "FilesHash": "1874ac1ecae98276",
            "Format": "3.0 (quilt)",
            "Homepage": "http://www.rust-lang.org/",
            "Key": "Psource rustc 1.48.0+dfsg1-2 1874ac1ecae98276",
            "Maintainer": "Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>",
            "Package": "rustc",
            "Package-List": " \n libstd-rust-1.48 deb libs optional arch=any\n libstd-rust-dev deb libdevel optional arch=any\n libstd-rust-dev-wasm32 deb libdevel optional arch=all profile=!nowasm\n libstd-rust-dev-windows deb libdevel optional arch=amd64 profile=!nowindows\n rust-doc deb doc optional arch=all profile=!nodoc\n rust-gdb deb devel optional arch=all\n rust-lldb deb devel optional arch=all\n rust-src deb devel optional arch=all\n rustc deb devel optional arch=any\n",
            "Priority": "optional",
            "Section": "rust",
            "ShortKey": "Psource rustc 1.48.0+dfsg1-2",
            "Standards-Version": "4.2.1",
            "Uploaders": "Ximin Luo <infinity0@debian.org>, Sylvestre Ledru <sylvestre@debian.org>",
            "Vcs-Browser": "https://salsa.debian.org/rust-team/rust",
            "Vcs-Git": "https://salsa.debian.org/rust-team/rust.git",
            "Version": "1.48.0+dfsg1-2"
        })).unwrap();

        assert_eq!("rustc", v.package());
    }
}
