use thiserror::Error;

/// Crate errors.
#[derive(Debug, Error)]
pub(crate) enum SmaugError {
    /// Error parsing an [Address<NetworkUnchecked>] to an [Address<NetworkChecked>].
    #[error("Error parsing address into required network")]
    NetworkMismatch(#[from] bitcoin::address::ParseError),

    /// Error creating `EsploraClient`.
    #[error("Error creating `EsploraClient`")]
    EsploraClient(#[from] esplora_client::Error),
}
