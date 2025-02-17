/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("signal error: {0}")]
    Signal(std::io::Error),
    #[error("failed to read file: {0}: {1}")]
    ReadFile(PathBuf, std::io::Error),
    #[error("failed to write file: {0}: {1}")]
    WriteFile(PathBuf, std::io::Error),
    #[error("failed to load certificate: {0}: {1}")]
    LoadCert(PathBuf, reqwest::Error),
    #[error("failed to deserialize: {0}: {1}")]
    Deserialize(PathBuf, serde_json::Error),
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("failed to delete pit")]
    DeletePit,
    #[error("timestamp out of bounds: {0}")]
    TimestampOutOfBounds(i64),
    #[error("relation graph error: {0}: {1}")]
    RelationGraph(reqwest::Error, String),
}
