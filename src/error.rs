use thiserror::Error;

#[derive(Error, Debug)]
pub enum CclinkError {
    #[error("No keypair found. Run `cclink init` first.")]
    NoKeypairFound,

    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("Key file corrupted: {0}")]
    KeyCorrupted(String),

    #[error("Failed to write key file atomically")]
    AtomicWriteFailed(#[source] std::io::Error),

    #[error("Cannot determine home directory")]
    HomeDirNotFound,

    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Record deserialization failed: {0}")]
    RecordDeserializationFailed(String),
}
