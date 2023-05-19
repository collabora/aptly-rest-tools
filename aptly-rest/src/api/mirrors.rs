use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, NoneAsEmptyString};

use crate::{key::AptlyKey, AptlyRestError};

#[derive(Debug, Clone)]
pub struct MirrorApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) name: String,
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
    pub components: Vec<String>,
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
