use serde_json::json;
use wiremock::{Respond, ResponseTemplate};

use crate::AptlyRestMock;

pub(crate) struct DbCleanupResponder {
    mock: AptlyRestMock,
}

impl DbCleanupResponder {
    pub(crate) fn new(mock: AptlyRestMock) -> Self {
        Self { mock }
    }
}

impl Respond for DbCleanupResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let mut async_mode = false;
        for (k, v) in request.url.query_pairs() {
            if k == "_async" && v == "true" {
                async_mode = true;
            }
        }

        let mut inner = self.mock.inner.write().unwrap();
        if inner.cleanup_running {
            return ResponseTemplate::new(409)
                .set_body_json(json!({"error": "Needed resources are used by other tasks."}));
        }
        inner.cleanup_running = true;

        if async_mode {
            ResponseTemplate::new(202).set_body_json(json!({
                "ID": 1,
                "Name": "Clean up db",
                "State": 0
            }))
        } else {
            inner.cleanup_running = false;
            ResponseTemplate::new(200).set_body_json(json!({}))
        }
    }
}
