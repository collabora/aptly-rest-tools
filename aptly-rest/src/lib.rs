use api::{
    packages::PackagesApi,
    repos::{Repo, RepoApi},
};
use serde::Deserialize;
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

    pub async fn repos(&self) -> Result<Vec<Repo>, AptlyRestError> {
        let url = self.url(&["api", "repos"]);
        self.get(url).await
    }

    pub fn repo<S: Into<String>>(&self, name: S) -> RepoApi {
        RepoApi {
            aptly: self,
            name: name.into(),
        }
    }

    pub fn packages(&self) -> PackagesApi {
        PackagesApi { aptly: self }
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

    async fn get<'a, T>(&self, url: Url) -> Result<T, AptlyRestError>
    where
        T: serde::de::DeserializeOwned,
    {
        let r = self.client.get(url).send().await?.error_for_status()?;
        Ok(r.json().await?)
    }
}
