//! Shellumi Server v1 integration: protocol wire models, cryptographic verification,
//! and authenticated HTTP API client.

pub mod client;
pub mod crypto;
pub mod error;
pub mod types;

#[cfg(test)]
mod tests;

pub use client::ShellumiServerClient;
pub use crypto::*;
pub use error::ServerError;
pub use types::*;
