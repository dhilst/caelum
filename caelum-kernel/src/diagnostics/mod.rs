use thiserror::Error;

/// Errors surfaced by the kernel. All variants are environment-agnostic: paths
/// are plain strings (a canonical filesystem path in the CLI, a virtual module
/// id in wasm) and IO failures are flattened to a message, so the kernel never
/// depends on `std::io` or `std::path` semantics.
#[derive(Debug, Error)]
pub enum CaelumError {
    #[error("failed to read {path}: {message}")]
    ReadFile { path: String, message: String },

    #[error("expected a .lum file, got {path}")]
    InvalidExtension { path: String },

    #[error("import error: {message}")]
    Import { message: String },

    #[error("parse error in {path}: {message}")]
    Parse { path: String, message: String },

    #[error("semantic error: {message}")]
    Semantic { message: String },

    #[error("model construction error: {message}")]
    Model { message: String },

    #[error("unsupported operation: {message}")]
    Unsupported { message: String },
}

pub type Result<T> = std::result::Result<T, CaelumError>;
