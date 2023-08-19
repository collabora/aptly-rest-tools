use std::{
    collections::HashMap,
    net::SocketAddr,
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
use clap::Parser;
use color_eyre::{eyre::WrapErr, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use thiserror::Error;
use tracing::{error, info};
use tracing::{metadata::LevelFilter, warn};
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

type LatestSnapshotsByDist = HashMap<String, String>;
type LockedLatestSnapshotsByDist = Arc<RwLock<LatestSnapshotsByDist>>;

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

async fn periodic_snapshot_refresh(
    locked_snapshots: LockedLatestSnapshotsByDist,
    aptly: AptlyRest,
    interval: Duration,
) {
    loop {
        tokio::time::sleep(interval).await;

        info!("Running periodic snapshots refresh...");
        match retrieve_latest_snapshots_by_dist(&aptly).await {
            Ok(snapshots) => {
                *locked_snapshots.write().unwrap() = snapshots;
                info!("Refresh complete.");
            }
            Err(err) => error!("Failed to refresh snapshots: {:?}", err),
        }
    }
}

#[derive(Clone)]
struct AppState {
    latest_snapshots_by_dist: LockedLatestSnapshotsByDist,
}

#[derive(TypedPath)]
#[typed_path("/healthz")]
struct Healthz;

async fn get_healthz(Healthz: Healthz) -> String {
    "OK".to_owned()
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/latest/:dist")]
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

#[derive(Parser, Debug)]
struct Opts {
    /// Url for the aptly rest api endpoint
    #[clap(
        short = 'u',
        long,
        env = "APTLY_API_URL",
        default_value = "http://localhost:8080"
    )]
    api_url: url::Url,
    /// Authentication token for the API
    #[clap(long, env = "APTLY_API_TOKEN")]
    api_token: Option<String>,
    /// Address and port to bind to
    #[clap(long = "bind-to", default_value = "0.0.0.0:8080")]
    bind_addr: SocketAddr,
    /// How often to refresh the latest snapshots
    #[clap(
        long,
        default_value_t = 600,
        value_parser = clap::value_parser!(u16).range(1..))]
    refresh_interval_sec: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();
    color_eyre::install().unwrap();

    let opts = Opts::parse();
    let aptly = if let Some(token) = &opts.api_token {
        AptlyRest::new_with_token(opts.api_url.clone(), token)?
    } else {
        AptlyRest::new(opts.api_url.clone())
    };

    info!("Retreiving latest snapshots...");
    let latest_snapshots_by_dist = Arc::new(RwLock::new(
        retrieve_latest_snapshots_by_dist(&aptly).await?,
    ));

    let refresh_handle = tokio::task::spawn(periodic_snapshot_refresh(
        latest_snapshots_by_dist.clone(),
        aptly.clone(),
        Duration::from_secs(opts.refresh_interval_sec as u64),
    ));

    let app = Router::new()
        .typed_get(get_healthz)
        .typed_get(get_latest_snapshot)
        .with_state(AppState {
            latest_snapshots_by_dist,
        });

    let server = axum::Server::try_bind(&opts.bind_addr)?;
    info!("Starting server on {}...", opts.bind_addr);

    tokio::select! {
        r = server.serve(app.into_make_service()) => {
            Err(r.wrap_err("Failed to run server").unwrap_err())
        }
        r = refresh_handle => {
            Err(r.wrap_err("Failed to run refresh task").unwrap_err())
        }
    }
}
