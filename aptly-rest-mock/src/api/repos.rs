use serde_json::json;
use wiremock::{Respond, ResponseTemplate};

use crate::AptlyRestMock;

pub(crate) struct ReposResponder {
    mock: AptlyRestMock,
}

impl ReposResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for ReposResponder {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let inner = self.mock.inner.read().unwrap();
        let reply: Vec<_> = inner
            .repositories
            .into_iter()
            .map(|r| {
                json!({
                      "Name": r.name,
                      "Comment": r.comment,
                      "DefaultDistribution": r.distribution,
                      "DefaultComponent": r.component,
                    }
                )
            })
            .collect();

        ResponseTemplate::new(200).set_body_json(reply)
    }
}

pub(crate) struct ReposPackagesResponder {
    mock: AptlyRestMock,
}

impl ReposPackagesResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for ReposPackagesResponder {
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
