//! Error types for Shellumi Server communication, protocol decoding, and crypto verification.

use crate::server::crypto::CryptoError;
use crate::server::types::ErrorResponse;
use crate::sync::SyncError;
use std::fmt;

/// High-level errors originating from Shellumi Server API interactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerError {
    /// Server returned an HTTP error without a standard API JSON error payload.
    Http(u16, String),
    /// Server returned a structured JSON `ErrorResponse`.
    Api(ErrorResponse),
    /// Underlying transport or network failure.
    Network(String),
    /// JSON serialization failure on outgoing request.
    Serialization(String),
    /// JSON deserialization failure on incoming response.
    Deserialization(String),
    /// Cryptographic validation or signature error.
    Crypto(String),
    /// Malformed base URL or endpoint URI.
    InvalidUrl(String),
    /// Authentication credentials missing or rejected (HTTP 401).
    Unauthorized,
    /// Resource not found (HTTP 404).
    NotFound(String),
    /// State or revision conflict (HTTP 409).
    Conflict(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(code, body) => write!(f, "HTTP error {code}: {body}"),
            Self::Api(err) => write!(f, "API error [{}]: {}", err.error_code, err.message),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
            Self::Deserialization(msg) => write!(f, "deserialization error: {msg}"),
            Self::Crypto(msg) => write!(f, "cryptographic error: {msg}"),
            Self::InvalidUrl(url) => write!(f, "invalid server URL: {url}"),
            Self::Unauthorized => {
                f.write_str("unauthorized: missing or invalid authentication token")
            }
            Self::NotFound(resource) => write!(f, "not found: {resource}"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<CryptoError> for ServerError {
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err.to_string())
    }
}

impl From<SyncError> for ServerError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Network(msg) => Self::Network(msg),
            SyncError::Http(code, msg) => Self::from_http_status_and_body(code, msg.as_bytes()),
            SyncError::Serialization(msg) => Self::Serialization(msg),
            SyncError::Deserialization(msg) => Self::Deserialization(msg),
            other => Self::Network(other.to_string()),
        }
    }
}

impl ServerError {
    /// Constructs a `ServerError` by attempting to parse an `ErrorResponse` from the response body,
    /// or falling back to a status-specific error variant.
    pub fn from_http_status_and_body(status: u16, body: &[u8]) -> Self {
        if let Ok(api_err) = serde_json::from_slice::<ErrorResponse>(body) {
            match status {
                401 => Self::Unauthorized,
                404 => Self::NotFound(api_err.message),
                409 => Self::Conflict(api_err.message),
                _ => Self::Api(api_err),
            }
        } else {
            let body_str = String::from_utf8_lossy(body).trim().to_string();
            match status {
                401 => Self::Unauthorized,
                404 => Self::NotFound(body_str),
                409 => Self::Conflict(body_str),
                _ => Self::Http(status, body_str),
            }
        }
    }

    /// Returns the machine-readable error code if this is an API error response.
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::Api(err) => Some(&err.error_code),
            _ => None,
        }
    }

    /// Whether this error indicates an authentication/token rejection.
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }

    /// Whether this error represents an optimistic concurrency / revision conflict.
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict(_))
    }

    /// Whether this error represents a resource not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }
}
