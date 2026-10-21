// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Server entry point. Configuration comes from `TPT_RUC_*` environment variables.
//!
//! `tpt-ruc-backend healthcheck` exits 0 when a server on `TPT_RUC_BIND` is
//! healthy, and 1 otherwise. The Docker image uses it for its health check.

use std::error::Error;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use tokio::net::TcpListener;
use tpt_ruc_backend::apikey::{self, KeySource};
use tpt_ruc_backend::config::{Config, DEFAULT_BIND};
use tpt_ruc_backend::data::run_retention;
use tpt_ruc_backend::db::Database;
use tpt_ruc_backend::eruc::{push_provider_named, PushProvider};
use tpt_ruc_backend::eruc_jobs::run_worker;
use tpt_ruc_backend::healthcheck;
use tpt_ruc_backend::ratelimit::RateLimiter;
use tpt_ruc_backend::server::serve;
use tpt_ruc_backend::AppState;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck_exit_code();
    }
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tpt-ruc-backend: {error}");
            ExitCode::FAILURE
        }
    }
}

fn healthcheck_exit_code() -> ExitCode {
    let bind = std::env::var("TPT_RUC_BIND")
        .unwrap_or_else(|_| DEFAULT_BIND.to_owned())
        .parse::<SocketAddr>()
        .unwrap_or_else(|_| DEFAULT_BIND.parse().unwrap());
    if healthcheck::check(bind) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    if let Some(dir) = &config.pwa_dir {
        if !dir.is_dir() {
            return Err(format!("TPT_RUC_PWA_DIR is not a folder: {}", dir.display()).into());
        }
        tracing::info!(path = %dir.display(), "serving the web app");
    }
    let db = Database::open(&config.database_url)?;
    let key = apikey::resolve(config.api_key.clone(), &config.data_dir())?;
    match &key.source {
        KeySource::Environment => tracing::info!("using the API key from TPT_RUC_API_KEY"),
        KeySource::File(path) => tracing::info!(path = %path.display(), "using the saved API key"),
        KeySource::Generated(path) => println!(
            "\nNo API key was set, so one was generated and saved to:\n  {}\n\n\
             Use it as the Bearer token:\n  Authorization: Bearer {}\n\n\
             Keep it secret. To use your own key instead, set TPT_RUC_API_KEY.\n",
            path.display(),
            key.key
        ),
    }

    let eruc_push: Option<Arc<dyn PushProvider>> =
        match &config.eruc_provider {
            Some(name) => Some(push_provider_named(name).ok_or_else(|| {
                format!("TPT_RUC_ERUC_PROVIDER names an unknown provider: {name}")
            })?),
            None => None,
        };

    let state = Arc::new(AppState {
        db,
        api_key: key.key,
        cors_origins: config.cors_origins,
        limiter: RateLimiter::default(),
        pwa_dir: config.pwa_dir,
        eruc_push: eruc_push.clone(),
    });

    if let Some(provider) = eruc_push {
        tracing::info!(provider = provider.id(), "eRUC push is on");
        tokio::spawn(run_worker(
            Arc::clone(&state),
            provider,
            std::time::Duration::from_secs(60),
        ));
    }

    if let Some(days) = config.retention_days {
        tracing::info!(days, "automatic retention is on");
        tokio::spawn(run_retention(
            Arc::clone(&state),
            days,
            std::time::Duration::from_secs(60 * 60),
        ));
    }

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "listening");
    serve(listener, state, shutdown_signal()).await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "could not listen for Ctrl+C");
    }
    tracing::info!("shutting down");
}
