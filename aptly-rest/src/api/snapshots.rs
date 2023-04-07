use reqwest::Url;
use serde::Deserialize;

use crate::AptlyRestError;

#[derive(Debug, Clone)]
pub struct SnapshotApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) name: String,
}

impl SnapshotApi<'_> {
    fn url(&self) -> Url {
        self.aptly.url(&["api", "snapshots", &self.name])
    }

    pub async fn get(&self) -> Result<Snapshot, AptlyRestError> {
        self.aptly.get(self.url()).await
    }

    pub async fn delete(&self, options: &DeleteOptions) -> Result<(), AptlyRestError> {
        let mut url = self.url();

        {
            let mut pairs = url.query_pairs_mut();
            if options.force {
                pairs.append_pair("force", "1");
            }
        }

        self.aptly
            .send_request(self.aptly.client.delete(url))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Snapshot {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
}

impl Snapshot {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }
}

#[derive(Debug, Default, Clone)]
pub struct DeleteOptions {
    pub force: bool,
}
