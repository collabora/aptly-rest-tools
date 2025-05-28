use std::{net::SocketAddr, time::Duration};

use aptly_latest_snapshots::{create_app, periodic_snapshot_refresh, AppState};
use aptly_rest::AptlyRest;
use clap::Parser;
use color_eyre::{eyre::WrapErr, Result};
use tracing::info;
use tracing::metadata::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

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

    let state = AppState::new(&aptly).await?;

    let refresh_handle = tokio::task::spawn(periodic_snapshot_refresh(
        state.clone(),
        aptly.clone(),
        Duration::from_secs(opts.refresh_interval_sec as u64),
    ));

    let app = create_app(state);

    let listener = tokio::net::TcpListener::bind(&opts.bind_addr).await?;
    info!("Starting server on {}...", opts.bind_addr);

    tokio::select! {
        r = axum::serve(listener, app.into_make_service()) => {
            Err(r.wrap_err("Failed to run server").unwrap_err())
        }
        r = refresh_handle => {
            Err(r.wrap_err("Failed to run refresh task").unwrap_err())
        }
    }
}
