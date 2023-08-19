use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeAs, NoneAsEmptyString, SerializeAs};

use crate::AptlyRestError;

#[derive(Debug, Clone)]
pub struct PublishApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
    pub(crate) prefix: String,
}

impl PublishApi<'_> {
    fn url(&self) -> Url {
        self.aptly.url(&["api", "publish", &self.prefix])
    }

    pub fn distribution<S: Into<String>>(&self, distribution: S) -> DistributionApi {
        DistributionApi {
            publish: self,
            distribution: distribution.into(),
        }
    }

    pub async fn publish(
        &self,
        kind: SourceKind,
        sources: &[Source],
        options: &PublishOptions,
    ) -> Result<PublishedRepo, AptlyRestError> {
        #[derive(Debug, Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct PublishRequest<'options> {
            #[serde(rename = "SourceKind")]
            kind: SourceKind,
            sources: &'options [Source],
            #[serde(flatten)]
            options: &'options PublishOptions,
        }

        self.aptly
            .post_body(
                self.url(),
                &PublishRequest {
                    kind,
                    sources,
                    options,
                },
            )
            .await
    }
}

#[derive(Debug, Clone)]
pub struct DistributionApi<'a> {
    pub(crate) publish: &'a PublishApi<'a>,
    pub(crate) distribution: String,
}

impl DistributionApi<'_> {
    fn url(&self) -> Url {
        self.publish
            .aptly
            .url(&["api", "publish", &self.publish.prefix, &self.distribution])
    }

    pub async fn update(&self, options: &UpdateOptions) -> Result<PublishedRepo, AptlyRestError> {
        self.publish.aptly.put_body(self.url(), options).await
    }

    pub async fn delete(&self, options: &DeleteOptions) -> Result<(), AptlyRestError> {
        let mut url = self.url();

        {
            let mut pairs = url.query_pairs_mut();
            if options.force {
                pairs.append_pair("force", "1");
            }
        }

        self.publish
            .aptly
            .send_request(self.publish.aptly.client.delete(url))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SigningOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpg_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_keyring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase_file: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Signing {
    Enabled(SigningOptions),
    Disabled,
}

impl Serialize for Signing {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct SigningOptionsSerializable<'options> {
            skip: bool,
            batch: bool,
            #[serde(flatten)]
            options: Option<&'options SigningOptions>,
        }

        match self {
            Signing::Enabled(options) => SigningOptionsSerializable {
                skip: false,
                batch: options.passphrase.is_some() || options.passphrase_file.is_some(),
                options: Some(options),
            },
            Signing::Disabled => SigningOptionsSerializable {
                skip: true,
                batch: false,
                options: None,
            },
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Source {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
}

struct YesNoBool;

impl SerializeAs<bool> for YesNoBool {
    fn serialize_as<S>(source: &bool, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(if *source { "yes" } else { "" })
    }
}

impl<'de> DeserializeAs<'de, bool> for YesNoBool {
    fn deserialize_as<D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "yes" => Ok(true),
            "" | "no" => Ok(false),
            _ => Err(serde::de::Error::custom("invalid yes/no value")),
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Snapshot,
    Local,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublishedRepo {
    #[serde(rename = "Storage")]
    #[serde_as(as = "NoneAsEmptyString")]
    storage_kind: Option<String>,
    prefix: String,
    distribution: String,
    source_kind: SourceKind,
    sources: Vec<Source>,
    architectures: Vec<String>,
    label: String,
    origin: String,
    #[serde_as(as = "YesNoBool")]
    not_automatic: bool,
    #[serde_as(as = "YesNoBool")]
    but_automatic_upgrades: bool,
    acquire_by_hash: bool,
}

impl PublishedRepo {
    pub fn storage_kind(&self) -> Option<&str> {
        self.storage_kind.as_deref()
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn distribution(&self) -> &str {
        &self.distribution
    }

    pub fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub fn architectures(&self) -> &[String] {
        &self.architectures
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn not_automatic(&self) -> bool {
        self.not_automatic
    }

    pub fn but_automatic_upgrades(&self) -> bool {
        self.but_automatic_upgrades
    }

    pub fn acquire_by_hash(&self) -> bool {
        self.acquire_by_hash
    }
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublishOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub force_overwrite: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing: Option<Signing>,
    #[serde_as(as = "YesNoBool")]
    pub not_automatic: bool,
    #[serde_as(as = "YesNoBool")]
    pub but_automatic_upgrades: bool,
    pub skip_cleanup: bool,
    pub acquire_by_hash: bool,
    pub skip_contents: bool,
    pub skip_bz2: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshots: Option<Vec<Source>>,
    pub force_overwrite: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing: Option<Signing>,
    pub acquire_by_hash: bool,
    pub skip_contents: bool,
    pub skip_bz2: bool,
}

#[derive(Debug, Default, Clone)]
pub struct DeleteOptions {
    pub force: bool,
}
