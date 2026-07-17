use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnNull, NoneAsEmptyString};

use crate::AptlyRestError;

#[derive(Debug, Clone)]
pub struct MirrorApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) name: String,
}

impl<'a> MirrorApi<'a> {
    pub fn url(&self) -> Url {
        self.aptly.url(&["api", "mirrors", &self.name])
    }

    pub fn create(&self, archive_url: Url) -> MirrorCreation<'_> {
        let request = MirrorCreateRequest::new(&self.name, archive_url);
        MirrorCreation {
            mirror: self,
            request,
        }
    }

    pub fn update(&self) -> MirrorUpdate<'_> {
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
    #[serde(rename = "ArchiveURL")]
    archive_url: Url,
    #[serde(skip_serializing_if = "Option::is_none")]
    distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    architectures: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keyrings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_signatures: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_sources: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_udebs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_installer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_app_stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_with_deps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_component_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_architecture_check: Option<bool>,
}

impl<'a> MirrorCreateRequest<'a> {
    fn new(name: &'a str, archive_url: Url) -> Self {
        MirrorCreateRequest {
            name,
            archive_url,
            distribution: None,
            filter: None,
            components: Vec::new(),
            architectures: Vec::new(),
            keyrings: Vec::new(),
            ignore_signatures: None,
            download_sources: None,
            download_udebs: None,
            download_installer: None,
            download_app_stream: None,
            filter_with_deps: None,
            skip_component_check: None,
            skip_architecture_check: None,
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

    pub fn filter<F: Into<String>>(&mut self, filter: F) -> &mut Self {
        self.request.filter = Some(filter.into());
        self
    }

    pub fn components(&mut self, components: Vec<String>) -> &mut Self {
        self.request.components = components;
        self
    }

    pub fn architectures(&mut self, architectures: Vec<String>) -> &mut Self {
        self.request.architectures = architectures;
        self
    }

    pub fn keyrings(&mut self, keyrings: Vec<String>) -> &mut Self {
        self.request.keyrings = keyrings;
        self
    }

    pub fn download_udebs(&mut self, v: bool) -> &mut Self {
        self.request.download_udebs = Some(v);
        self
    }

    pub fn download_installer(&mut self, v: bool) -> &mut Self {
        self.request.download_installer = Some(v);
        self
    }

    pub fn download_app_stream(&mut self, v: bool) -> &mut Self {
        self.request.download_app_stream = Some(v);
        self
    }

    pub fn filter_with_deps(&mut self, v: bool) -> &mut Self {
        self.request.filter_with_deps = Some(v);
        self
    }

    pub fn skip_component_check(&mut self, v: bool) -> &mut Self {
        self.request.skip_component_check = Some(v);
        self
    }

    pub fn skip_architecture_check(&mut self, v: bool) -> &mut Self {
        self.request.skip_architecture_check = Some(v);
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keyrings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_checksums: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_signatures: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    force_update: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_existing_packages: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_only: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct MirrorUpdate<'a> {
    mirror: &'a MirrorApi<'a>,
    request: MirrorUpdateRequest,
}

impl MirrorUpdate<'_> {
    pub fn rename<N: Into<String>>(&mut self, name: N) -> &mut Self {
        self.request.name = Some(name.into());
        self
    }

    pub fn keyrings(&mut self, keyrings: Vec<String>) -> &mut Self {
        self.request.keyrings = keyrings;
        self
    }

    pub fn ignore_checksums(&mut self, v: bool) -> &mut Self {
        self.request.ignore_checksums = Some(v);
        self
    }

    pub fn ignore_signatures(&mut self, v: bool) -> &mut Self {
        self.request.ignore_signatures = Some(v);
        self
    }

    pub fn force_update(&mut self, v: bool) -> &mut Self {
        self.request.force_update = Some(v);
        self
    }

    pub fn skip_existing_packages(&mut self, v: bool) -> &mut Self {
        self.request.skip_existing_packages = Some(v);
        self
    }

    pub fn latest_only(&mut self, v: bool) -> &mut Self {
        self.request.latest_only = Some(v);
        self
    }

    pub async fn run(&self) -> Result<(), AptlyRestError> {
        self.mirror
            .aptly
            .send_request(
                self.mirror
                    .aptly
                    .client
                    .put(self.mirror.url())
                    .json(&self.request),
            )
            .await?;
        Ok(())
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mirror {
    #[serde(rename = "UUID")]
    uuid: String,
    name: String,
    archive_root: String,
    distribution: String,
    #[serde_as(as = "DefaultOnNull")]
    components: Vec<String>,
    #[serde_as(as = "DefaultOnNull")]
    architectures: Vec<String>,
    last_download_date: String,
    #[serde_as(as = "NoneAsEmptyString")]
    filter: Option<String>,
    status: u32,
    #[serde(rename = "WorkerPID")]
    worker_pid: u32,
    filter_with_deps: bool,
    skip_component_check: bool,
    skip_architecture_check: bool,
    download_sources: bool,
    download_udebs: bool,
    download_installer: bool,
    meta: HashMap<String, String>,
}

impl Mirror {
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn archive_root(&self) -> &str {
        &self.archive_root
    }

    pub fn distribution(&self) -> &str {
        &self.distribution
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    pub fn architectures(&self) -> &[String] {
        &self.architectures
    }

    pub fn last_download_date(&self) -> &str {
        &self.last_download_date
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn status(&self) -> u32 {
        self.status
    }

    pub fn worker_pid(&self) -> u32 {
        self.worker_pid
    }

    pub fn filter_with_deps(&self) -> bool {
        self.filter_with_deps
    }

    pub fn skip_component_check(&self) -> bool {
        self.skip_component_check
    }

    pub fn skip_architecture_check(&self) -> bool {
        self.skip_architecture_check
    }

    pub fn download_sources(&self) -> bool {
        self.download_sources
    }

    pub fn download_udebs(&self) -> bool {
        self.download_udebs
    }

    pub fn download_installer(&self) -> bool {
        self.download_installer
    }

    pub fn meta(&self) -> &HashMap<String, String> {
        &self.meta
    }
}
