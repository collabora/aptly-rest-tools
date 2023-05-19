use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnNull, NoneAsEmptyString};

use crate::{key::AptlyKey, AptlyRestError};

#[derive(Debug, Clone)]
pub struct MirrorApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) name: String,
}

impl<'a> MirrorApi<'a> {
    pub fn url(&self) -> Url {
        self.aptly.url(&["api", "mirrors", &self.name])
    }
    pub fn create<U: Into<String>>(&self, archive_url: U) -> MirrorCreation {
        let request = MirrorCreateRequest::new(&self.name, archive_url.into());
        MirrorCreation {
            mirror: self,
            request,
        }
    }

    pub fn update(&self) -> MirrorUpdate {
        MirrorUpdate {
            mirror: self,
            request: Default::default(),
        }
    }

    pub async fn drop(self) -> Result<(), AptlyRestError> {
        self.aptly
            .send_request(self.aptly.client.delete(self.url()))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct MirrorCreateRequest<'a> {
    name: &'a str,
    archive_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_signatures: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_sources: Option<bool>,
}

impl<'a> MirrorCreateRequest<'a> {
    fn new(name: &'a str, archive_url: String) -> Self {
        MirrorCreateRequest {
            name,
            archive_url,
            distribution: None,
            ignore_signatures: None,
            download_sources: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorCreation<'a> {
    mirror: &'a MirrorApi<'a>,
    request: MirrorCreateRequest<'a>,
}

impl MirrorCreation<'_> {
    pub fn ignore_signatures(&mut self, v: bool) -> &mut Self {
        self.request.ignore_signatures = Some(v);
        self
    }

    pub fn download_sources(&mut self, v: bool) -> &mut Self {
        self.request.download_sources = Some(v);
        self
    }

    pub fn distribution<D: Into<String>>(&mut self, distribution: D) -> &mut Self {
        self.request.distribution = Some(distribution.into());
        self
    }

    pub async fn run(&self) -> Result<Mirror, AptlyRestError> {
        self.mirror
            .aptly
            .post_body(self.mirror.aptly.url(&["api", "mirrors"]), &self.request)
            .await
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "PascalCase")]
struct MirrorUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_signatures: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_sources: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct MirrorUpdate<'a> {
    mirror: &'a MirrorApi<'a>,
    request: MirrorUpdateRequest,
}

impl MirrorUpdate<'_> {
    pub fn ignore_signatures(&mut self, v: bool) -> &mut Self {
        self.request.ignore_signatures = Some(v);
        self
    }

    pub fn distribution<D: Into<String>>(&mut self, distribution: D) -> &mut Self {
        self.request.distribution = Some(distribution.into());
        self
    }

    pub fn archive_url<U: Into<String>>(&mut self, archive_url: U) -> &mut Self {
        self.request.archive_url = Some(archive_url.into());
        self
    }

    pub fn download_sources(&mut self, v: bool) -> &mut Self {
        self.request.download_sources = Some(v);
        self
    }

    pub async fn run(&self) -> Result<(), AptlyRestError> {
        self.mirror
            .aptly
            .put_body(self.mirror.url(), &self.request)
            .await
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mirror {
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub name: String,
    pub archive_root: String,
    pub distribution: String,
    #[serde_as(as = "DefaultOnNull")]
    pub components: Vec<String>,
    #[serde_as(as = "DefaultOnNull")]
    pub architectures: Vec<String>,
    last_download_date: String,
    #[serde_as(as = "NoneAsEmptyString")]
    filter: Option<String>,
    status: u32,
    #[serde(rename = "WorkerPID")]
    worker_pid: u32,
    filter_with_deps: bool,
    skip_component_check: bool,
    download_sources: bool,
    download_udebs: bool,
    download_installer: bool,
    meta: HashMap<String, String>,
}
