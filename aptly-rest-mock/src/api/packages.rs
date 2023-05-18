use wiremock::{Respond, ResponseTemplate};

use crate::AptlyRestMock;

pub(crate) struct PackagesResponder {
    #[allow(dead_code)]
    mock: AptlyRestMock,
}

impl PackagesResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for PackagesResponder {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!([]))
    }
}
