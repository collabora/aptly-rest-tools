use serde_json::json;
use wiremock::{Respond, ResponseTemplate};

use crate::AptlyRestMock;

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
            .map(|m| {
                let data = &m.data;
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
                })
            })
            .collect();

        ResponseTemplate::new(200).set_body_json(reply)
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
