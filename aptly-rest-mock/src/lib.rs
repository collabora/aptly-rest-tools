use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;

use http_types::StatusCode;
use mirror::{Mirror, Mirrors};
use pool::Package;
use repo::Repositories;
use serde::Deserialize;
use serde_json::json;
use url::Url;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;
use wiremock::ResponseTemplate;
use wiremock::{Mock, MockServer};

mod api;
mod mirror;
mod pool;
mod repo;
use pool::Pool;

pub const APTLY_VERSION: &str = "1.4.0+187+g15f2c97d";

struct Inner {
    pool: Pool,
    repositories: Repositories,
    mirrors: Mirrors,
}

#[derive(Clone)]
pub struct AptlyRestMock {
    server: Arc<MockServer>,
    inner: Arc<RwLock<Inner>>,
}

impl AptlyRestMock {
    pub async fn start() -> Self {
        let inner = Arc::new(RwLock::new(Inner {
            pool: Pool::new(),
            repositories: Repositories::new(),
            mirrors: Mirrors::new(),
        }));
        let server = AptlyRestMock {
            server: Arc::new(MockServer::start().await),
            inner,
        };

        Mock::given(method("GET"))
            .and(path("api/version"))
            .respond_with(
                ResponseTemplate::new(StatusCode::Ok)
                    .set_body_json(json!({ "Version": APTLY_VERSION })),
            )
            .mount(&server.server)
            .await;

        Mock::given(method("GET"))
            .and(path("api/packages"))
            .respond_with(api::packages::PackagesResponder::new(server.clone()))
            .mount(&server.server)
            .await;

        Mock::given(method("GET"))
            .and(path("api/repos"))
            .respond_with(api::repos::ReposResponder::new(server.clone()))
            .mount(&server.server)
            .await;

        Mock::given(method("GET"))
            .and(path_regex("api/repos/[^/]*/packages"))
            .respond_with(api::repos::ReposPackagesResponder::new(server.clone()))
            .mount(&server.server)
            .await;

        Mock::given(method("GET"))
            .and(path("api/mirrors"))
            .respond_with(api::mirrors::MirrorsResponder::new(server.clone()))
            .mount(&server.server)
            .await;

        server
    }

    /// Load mock data at a given path
    pub fn load_data(&self, path: &Path) {
        let f = File::open(path).expect("Couldn't open data");
        let mut data: Data = serde_json::from_reader(f).expect("Couldn't parse data");
        let mut inner = self.inner.write().unwrap();

        for p in data.packages {
            inner.pool.add_json_package(p)
        }

        for r in data.repositories {
            inner.repositories.add(
                r.name,
                r.comment,
                r.default_distribution,
                r.default_component,
            );
        }
        if let Some(mut mirrors) = data.mirrors {
            for m in mirrors.drain(..) {
                inner.mirrors.add(m.into());
            }
        }
        drop(inner);

        for c in data.contents {
            for p in c.packages {
                self.repo_add_package(&c.repository, p)
            }
        }
    }

    /// Load default set of packages and repositories for the mock
    pub fn load_default_data(&self) {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data/default-data.json");

        self.load_data(&path);
    }

    /// Add package to named repository using aptly key.
    ///
    /// The package with the given key should already be in the package pool
    /// and the repository should be part of the repositories
    pub fn repo_add_package(&self, repo: &str, key: String) {
        let mut inner = self.inner.write().unwrap();
        assert!(inner.pool.has_package(&key), "{} not found in pool", key);
        inner.repositories.add_package(repo, key);
    }

    pub fn url(&self) -> Url {
        self.server.uri().parse().expect("uri is not a url")
    }

    pub fn repos(&self) -> Repositories {
        let inner = self.inner.read().unwrap();
        inner.repositories.clone()
    }

    pub fn mirrors(&self) -> Mirrors {
        let inner = self.inner.read().unwrap();
        inner.mirrors.clone()
    }

    pub fn package(&self, key: &str) -> Option<Package> {
        let inner = self.inner.read().unwrap();
        inner.pool.package(key).cloned()
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct RepoData {
    name: String,
    comment: String,
    default_distribution: String,
    default_component: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct MirrorData {
    #[serde(rename = "UUID")]
    uuid: String,
    name: String,
    archive_root: String,
    distribution: String,
    components: Vec<String>,
    architectures: Vec<String>,
    meta: HashMap<String, String>,
    last_download_date: String,
    filter: String,
    status: u32,
    #[serde(rename = "WorkerPID")]
    worker_pid: u32,
    filter_with_deps: bool,
    skip_component_check: bool,
    skip_architecture_check: bool,
    download_sources: bool,
    download_udebs: bool,
    download_installer: bool,
}

#[derive(Deserialize, Debug)]
struct ContentData {
    repository: String,
    packages: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct Data {
    repositories: Vec<RepoData>,
    mirrors: Option<Vec<MirrorData>>,
    contents: Vec<ContentData>,
    packages: Vec<serde_json::Value>,
}
