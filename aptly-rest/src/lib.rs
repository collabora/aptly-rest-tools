use api::{
    files::FilesApi,
    packages::PackagesApi,
    publish::{PublishApi, PublishedRepo},
    repos::{Repo, RepoApi},
    snapshots::{Snapshot, SnapshotApi},
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

pub mod api;
pub mod changes;
pub mod dsc;
pub mod key;
pub mod utils;

#[derive(Error, Debug)]
pub enum AptlyRestError {
    #[error("Http Request failed {0}")]
    Request(#[from] reqwest::Error),
    #[error("Invalid authentication token {0}")]
    InvalidAuthToken(#[from] header::InvalidHeaderValue),
}

#[derive(Debug, Clone)]
pub struct AptlyRest {
    client: reqwest::Client,
    url: Url,
}

impl AptlyRest {
    pub fn new(url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
        }
    }

    pub fn new_with_token(url: Url, token: &str) -> Result<Self, AptlyRestError> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, format!("Bearer {token}").parse()?);

        Ok(Self {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()?,
            url,
        })
    }

    pub async fn version(&self) -> Result<String, AptlyRestError> {
        let mut url = self.url.clone();
        url.path_segments_mut().unwrap().extend(&["api", "version"]);

        let r = self.client.get(url).send().await?.error_for_status()?;

        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Version {
            version: String,
        }
        let v: Version = r.json().await?;
        Ok(v.version)
    }

    pub async fn db_cleanup(&self) -> Result<(), AptlyRestError> {
        let url = self.url(&["api", "db", "cleanup"]);
        self.post::<()>(url).await?;
        Ok(())
    }

    pub async fn repos(&self) -> Result<Vec<Repo>, AptlyRestError> {
        let url = self.url(&["api", "repos"]);
        self.get(url).await
    }

    pub async fn create_repo(&self, repo: &Repo) -> Result<Repo, AptlyRestError> {
        let url = self.url(&["api", "repos"]);
        self.post_body(url, repo).await
    }

    pub fn repo<S: Into<String>>(&self, name: S) -> RepoApi<'_> {
        RepoApi {
            aptly: self,
            name: name.into(),
        }
    }

    pub fn files(&self) -> FilesApi<'_> {
        FilesApi { aptly: self }
    }

    pub fn packages(&self) -> PackagesApi<'_> {
        PackagesApi { aptly: self }
    }

    pub fn publish_prefix<S: Into<String>>(&self, prefix: S) -> PublishApi<'_> {
        PublishApi {
            aptly: self,
            prefix: prefix.into(),
        }
    }

    pub async fn published(&self) -> Result<Vec<PublishedRepo>, AptlyRestError> {
        let url = self.url(&["api", "publish"]);
        self.get(url).await
    }

    pub fn snapshot<S: Into<String>>(&self, name: S) -> SnapshotApi<'_> {
        SnapshotApi {
            aptly: self,
            name: name.into(),
        }
    }

    pub async fn snapshots(&self) -> Result<Vec<Snapshot>, AptlyRestError> {
        let url = self.url(&["api", "snapshots"]);
        self.get(url).await
    }

    fn url<I>(&self, parts: I) -> Url
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut url = self.url.clone();
        url.path_segments_mut().unwrap().extend(parts);
        url
    }

    async fn get<T>(&self, url: Url) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.json_request(self.client.get(url)).await
    }

    async fn post<T>(&self, url: Url) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.json_request(self.client.post(url)).await
    }

    async fn post_body<S: Serialize + ?Sized, T>(
        &self,
        url: Url,
        body: &S,
    ) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.json_request(self.client.post(url).json(body)).await
    }

    async fn put_body<S: Serialize + ?Sized, T>(
        &self,
        url: Url,
        body: &S,
    ) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.json_request(self.client.put(url).json(body)).await
    }

    async fn send_request(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, AptlyRestError> {
        Ok(req.send().await?.error_for_status()?)
    }

    async fn json_request<T>(&self, req: reqwest::RequestBuilder) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(self.send_request(req).await?.json().await?)
    }
}
