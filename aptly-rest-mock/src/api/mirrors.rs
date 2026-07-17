use serde::Deserialize;
use serde_json::{json, Value};
use wiremock::{Respond, ResponseTemplate};

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
