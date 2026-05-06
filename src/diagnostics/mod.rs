use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TplError {
    #[error("failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("expected a .tpl file, got {path}")]
    InvalidExtension { path: PathBuf },

    #[error("import error: {message}")]
    Import { message: String },

    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("semantic error: {message}")]
    Semantic { message: String },

    #[error("model construction error: {message}")]
    Model { message: String },

    #[error("unsupported operation: {message}")]
    Unsupported { message: String },
}

pub type Result<T> = std::result::Result<T, TplError>;
