/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

mod discovery;
mod error;
mod query;
mod state;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use clap::Parser;
use discovery::Discovery;
use flate2::{read::GzDecoder, Compression};
use reqwest::{Certificate, Identity};
use serde::{de::DeserializeOwned, Serialize};
use url::Url;

use crate::error::Error;

#[derive(Parser)]
struct Args {
    #[clap(long)]
    es_url: Url,
    #[clap(long)]
    es_ca: PathBuf,
    #[clap(long)]
    es_cert: PathBuf,
    #[clap(long)]
    es_key: PathBuf,
    #[clap(long)]
    rg_url: Url,
    #[clap(long, short, default_value = "60", help = "interval in seconds")]
    interval: u64,
    #[clap(long, short)]
    state: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    env_logger::init();
    let args = Args::parse();
    if let Err(e) = run(&args).await {
        log::error!("{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn run(args: &Args) -> Result<(), Error> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(Error::Signal)?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(Error::Signal)?;
    let mut interval = tokio::time::interval(Duration::from_secs(args.interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut discovery = Discovery::new(args).await?;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = sigterm.recv() => {
                log::info!("caught SIGTERM; shutting down...");
                return Ok(())
            }
            _ = sigint.recv() => {
                log::info!("caught SIGINT; shutting down...");
                return Ok(())
            }
        }

        if let Err(e) = discovery.discover().await {
            log::warn!("discovery failed: {e}");
        }
    }
}

async fn load_cert(path: &Path) -> Result<Certificate, Error> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| Error::ReadFile(path.to_path_buf(), e))?;
    Certificate::from_pem(&data).map_err(|e| Error::LoadCert(path.to_path_buf(), e))
}

async fn load_identity(cert_path: &Path, key_path: &Path) -> Result<Identity, Error> {
    let cert_data = tokio::fs::read(cert_path)
        .await
        .map_err(|e| Error::ReadFile(cert_path.to_path_buf(), e))?;
    let key_data = tokio::fs::read(key_path)
        .await
        .map_err(|e| Error::ReadFile(key_path.to_path_buf(), e))?;
    Identity::from_pkcs8_pem(&cert_data, &key_data)
        .map_err(|e| Error::LoadCert(cert_path.to_path_buf(), e))
}

async fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| Error::ReadFile(path.to_path_buf(), e))?;
    serde_json::from_reader(GzDecoder::new(data.as_slice()))
        .map_err(|e| Error::Deserialize(path.to_path_buf(), e))
}

async fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let mut data = Vec::new();
    serde_json::to_writer(
        flate2::write::GzEncoder::new(&mut data, Compression::fast()),
        value,
    )
    .unwrap();
    tokio::fs::write(path, &data)
        .await
        .map_err(|e| Error::WriteFile(path.to_path_buf(), e))
}
