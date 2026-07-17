use serde::Deserialize;
use serde_json::{json, Value};
use wiremock::{Respond, ResponseTemplate};

use crate::mirror::AppliedUpdate;
use crate::{AptlyRestMock, MirrorData};

fn mirror_json(data: &MirrorData) -> Value {
    json!({
      "UUID": data.uuid,
      "Name": data.name,
      "ArchiveRoot": data.archive_root,
      "Distribution": data.distribution,
      "Components": data.components,
      "Architectures": data.architectures,
      "Meta": data.meta,
      "LastDownloadDate": data.last_download_date,
      "Filter": data.filter,
      "Status": data.status,
      "WorkerPID": data.worker_pid,
      "FilterWithDeps": data.filter_with_deps,
      "SkipComponentCheck": data.skip_component_check,
      "SkipArchitectureCheck": data.skip_architecture_check,
      "DownloadSources": data.download_sources,
      "DownloadUdebs": data.download_udebs,
      "DownloadInstaller": data.download_installer,
      "DownloadAppStream": data.download_app_stream,
    })
}

pub(crate) struct MirrorsResponder {
    mock: AptlyRestMock,
}

impl MirrorsResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for MirrorsResponder {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let inner = self.mock.inner.read().unwrap();
        let reply: Vec<_> = inner
            .mirrors
            .into_iter()
            .map(|m| mirror_json(&m.data))
            .collect();

        ResponseTemplate::new(200).set_body_json(reply)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MirrorCreateParams {
    name: String,
    // The client serializes this as PascalCase "ArchiveUrl"; real aptly sends
    // "ArchiveURL". Accept both.
    #[serde(alias = "ArchiveURL")]
    archive_url: String,
    #[serde(default)]
    distribution: String,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    keyrings: Vec<String>,
    #[serde(default)]
    ignore_signatures: bool,
    #[serde(default)]
    download_sources: bool,
    #[serde(default)]
    download_udebs: bool,
    #[serde(default)]
    download_installer: bool,
    #[serde(default)]
    download_app_stream: bool,
    #[serde(default)]
    filter_with_deps: bool,
    #[serde(default)]
    skip_component_check: bool,
    #[serde(default)]
    skip_architecture_check: bool,
}

impl From<MirrorCreateParams> for MirrorData {
    fn from(p: MirrorCreateParams) -> Self {
        MirrorData {
            uuid: format!("mock-uuid-{}", p.name),
            name: p.name,
            archive_root: p.archive_url,
            distribution: p.distribution,
            components: p.components,
            architectures: p.architectures,
            meta: Default::default(),
            last_download_date: String::new(),
            filter: p.filter,
            status: 0,
            worker_pid: 0,
            filter_with_deps: p.filter_with_deps,
            skip_component_check: p.skip_component_check,
            skip_architecture_check: p.skip_architecture_check,
            download_sources: p.download_sources,
            download_udebs: p.download_udebs,
            download_installer: p.download_installer,
            download_app_stream: p.download_app_stream,
            keyrings: p.keyrings,
            ignore_signatures: p.ignore_signatures,
        }
    }
}

pub(crate) struct MirrorsCreateResponder {
    mock: AptlyRestMock,
}

impl MirrorsCreateResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for MirrorsCreateResponder {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let params: MirrorCreateParams = match serde_json::from_slice(&request.body) {
            Ok(params) => params,
            Err(e) => {
                return ResponseTemplate::new(400).set_body_string(e.to_string());
            }
        };

        let data: MirrorData = params.into();
        let body = mirror_json(&data);

        let mut inner = self.mock.inner.write().unwrap();
        inner.mirrors.add(data.into());

        ResponseTemplate::new(201).set_body_json(body)
    }
}

// Deny unknown fields so that sending parameters the real update (PUT) endpoint does
// not accept -- e.g. config-edit fields, which belong to the separate edit endpoint --
// is caught rather than silently ignored.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct MirrorUpdateParams {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    keyrings: Vec<String>,
    #[serde(default)]
    ignore_checksums: bool,
    #[serde(default)]
    ignore_signatures: bool,
    #[serde(default)]
    force_update: bool,
    #[serde(default)]
    skip_existing_packages: bool,
    #[serde(default)]
    latest_only: bool,
}

impl From<MirrorUpdateParams> for AppliedUpdate {
    fn from(p: MirrorUpdateParams) -> Self {
        AppliedUpdate {
            rename: p.name,
            keyrings: p.keyrings,
            ignore_checksums: p.ignore_checksums,
            ignore_signatures: p.ignore_signatures,
            force_update: p.force_update,
            skip_existing_packages: p.skip_existing_packages,
            latest_only: p.latest_only,
        }
    }
}

pub(crate) struct MirrorsUpdateResponder {
    mock: AptlyRestMock,
}

impl MirrorsUpdateResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for MirrorsUpdateResponder {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let name = request
            .url
            .path_segments()
            .and_then(|mut s| s.nth(2))
            .unwrap_or("")
            .to_owned();

        let params: MirrorUpdateParams = match serde_json::from_slice(&request.body) {
            Ok(params) => params,
            Err(e) => {
                return ResponseTemplate::new(400).set_body_string(e.to_string());
            }
        };

        let mut inner = self.mock.inner.write().unwrap();
        match inner.mirrors.get_mut(&name) {
            Some(mirror) => {
                mirror.set_applied_update(params.into());
                ResponseTemplate::new(200)
            }
            None => ResponseTemplate::new(404),
        }
    }
}

// Deny unknown fields so that sending parameters the edit (POST) endpoint does not
// accept -- e.g. Components or the skip-checks, which are not editable -- is caught
// rather than silently ignored.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct MirrorEditParams {
    // The client serializes this as PascalCase "ArchiveUrl"; real aptly sends
    // "ArchiveURL". Accept both.
    #[serde(default, alias = "ArchiveURL")]
    archive_url: Option<String>,
    #[serde(default)]
    filter: Option<String>,
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    keyrings: Vec<String>,
    #[serde(default)]
    filter_with_deps: Option<bool>,
    #[serde(default)]
    download_sources: Option<bool>,
    #[serde(default)]
    download_udebs: Option<bool>,
    #[serde(default)]
    download_installer: Option<bool>,
    #[serde(default)]
    ignore_signatures: Option<bool>,
}

pub(crate) struct MirrorsEditResponder {
    mock: AptlyRestMock,
}

impl MirrorsEditResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for MirrorsEditResponder {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let name = request
            .url
            .path_segments()
            .and_then(|mut s| s.nth(2))
            .unwrap_or("")
            .to_owned();

        let params: MirrorEditParams = match serde_json::from_slice(&request.body) {
            Ok(params) => params,
            Err(e) => {
                return ResponseTemplate::new(400).set_body_string(e.to_string());
            }
        };

        let mut inner = self.mock.inner.write().unwrap();
        match inner.mirrors.get_mut(&name) {
            Some(mirror) => {
                let data = &mut mirror.data;
                if let Some(archive_url) = params.archive_url {
                    data.archive_root = archive_url;
                }
                if let Some(filter) = params.filter {
                    data.filter = filter;
                }
                if !params.architectures.is_empty() {
                    data.architectures = params.architectures;
                }
                if !params.keyrings.is_empty() {
                    data.keyrings = params.keyrings;
                }
                if let Some(v) = params.filter_with_deps {
                    data.filter_with_deps = v;
                }
                if let Some(v) = params.download_sources {
                    data.download_sources = v;
                }
                if let Some(v) = params.download_udebs {
                    data.download_udebs = v;
                }
                if let Some(v) = params.download_installer {
                    data.download_installer = v;
                }
                if let Some(v) = params.ignore_signatures {
                    data.ignore_signatures = v;
                }
                let body = mirror_json(data);
                ResponseTemplate::new(200).set_body_json(body)
            }
            None => ResponseTemplate::new(404),
        }
    }
}

/*
pub(crate) struct MirrorsPackagesResponder {
    mock: AptlyRestMock,
}

impl MirrorsPackagesResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for MirrorsPackagesResponder {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let name = request.url.path_segments().unwrap().nth(2).unwrap();

        let mut detailed = false;
        for (k, v) in request.url.query_pairs() {
            match (k.as_ref(), v.as_ref()) {
                ("format", "details") => detailed = true,
                (k, v) => unimplemented!("query pair {k}={v}"),
            }
        }

        let inner = self.mock.inner.read().unwrap();
        if let Some(repo) = inner.repositories.get(name) {
            if detailed {
                let packages: Vec<_> = repo
                    .packages()
                    .iter()
                    .map(|r| inner.pool.package(r).unwrap().fields())
                    .collect();

                ResponseTemplate::new(200).set_body_json(packages)
            } else {
                ResponseTemplate::new(200).set_body_json(repo.packages())
            }
        } else {
            ResponseTemplate::new(404)
        }
    }
}
*/
