use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use aptly_rest::{api::publish, AptlyRest};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
};
use axum_extra::routing::{RouterExt, TypedPath};
use color_eyre::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use thiserror::Error;
use tracing::{error, info, warn};

type LatestSnapshotsByDist = HashMap<String, String>;

#[derive(Error, Debug)]
enum AppError {
    #[error("dist {0} not found")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()).into_response(),
        }
    }
}

async fn retrieve_latest_snapshots_by_dist(aptly: &AptlyRest) -> Result<LatestSnapshotsByDist> {
    static SNAPSHOT_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?<dist>[^/]+)/snapshots/(?<timestamp>\d{8}T\d{6}Z)").unwrap());

    let mut latest_snapshots = HashMap::<String, String>::new();

    for publish in aptly
        .published()
        .await?
        .into_iter()
        .filter(|p| p.source_kind() == publish::SourceKind::Snapshot)
    {
        let Some(captures) = SNAPSHOT_RE.captures(publish.distribution()) else {
            warn!("Invalid snapshot publish: {}", publish.distribution());
            continue;
        };

        let dist = &captures["dist"];
        let timestamp = &captures["timestamp"];

        latest_snapshots
            .entry(dist.to_owned())
            .and_modify(|latest| {
                if timestamp > latest.as_str() {
                    *latest = timestamp.to_owned();
                }
            })
            .or_insert_with(|| timestamp.to_owned());
    }

    Ok(latest_snapshots)
}

#[derive(Clone)]
pub struct AppState {
    latest_snapshots_by_dist: Arc<RwLock<LatestSnapshotsByDist>>,
}

impl AppState {
    pub async fn new(aptly: &AptlyRest) -> Result<AppState> {
        info!("Retreiving latest snapshots...");
        let latest_snapshots_by_dist =
            Arc::new(RwLock::new(retrieve_latest_snapshots_by_dist(aptly).await?));
        Ok(Self {
            latest_snapshots_by_dist,
        })
    }
}

pub async fn periodic_snapshot_refresh(state: AppState, aptly: AptlyRest, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;

        info!("Running periodic snapshots refresh...");
        match retrieve_latest_snapshots_by_dist(&aptly).await {
            Ok(snapshots) => {
                *state.latest_snapshots_by_dist.write().unwrap() = snapshots;
                info!("Refresh complete.");
            }
            Err(err) => error!("Failed to refresh snapshots: {:?}", err),
        }
    }
}

#[derive(TypedPath)]
#[typed_path("/healthz")]
struct Healthz;

async fn get_healthz(Healthz: Healthz) -> String {
    "OK".to_owned()
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/latest/{dist}")]
struct LatestSnapshot {
    dist: String,
}

async fn get_latest_snapshot(
    LatestSnapshot { dist }: LatestSnapshot,
    State(state): State<AppState>,
) -> Result<String, AppError> {
    let latest_snapshots = state.latest_snapshots_by_dist.read().unwrap();
    if let Some(s) = latest_snapshots.get(&dist) {
        Ok(s.clone())
    } else {
        Err(AppError::NotFound(dist))
    }
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .typed_get(get_healthz)
        .typed_get(get_latest_snapshot)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use rstest::rstest;

    use super::*;

    const TEST_DIST: &str = "v2024";
    const TEST_SNAPSHOT: &str = "20241119T093902Z";

    #[rstest::fixture]
    fn server() -> TestServer {
        let app = create_app(AppState {
            latest_snapshots_by_dist: Arc::new(RwLock::new(
                [(TEST_DIST.to_owned(), TEST_SNAPSHOT.to_owned())].into(),
            )),
        });

        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    #[rstest]
    async fn test_healthz(server: TestServer) {
        server.get("/healthz").await.assert_status_success();
    }

    #[tokio::test]
    #[rstest]
    async fn test_latest_snapshot(server: TestServer) {
        let resp = server.get(&format!("/latest/{TEST_DIST}")).await;
        resp.assert_status_success();
        resp.assert_text(TEST_SNAPSHOT);
    }

    #[tokio::test]
    #[rstest]
    async fn test_latest_snapshot_missing(server: TestServer) {
        let resp = server.get("/latest/xyz123").await;
        resp.assert_status_not_found();
    }
}
