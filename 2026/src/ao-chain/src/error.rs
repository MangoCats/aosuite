use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("invalid genesis block: {0}")]
    InvalidGenesis(String),

    #[error("invalid assignment: {0}")]
    InvalidAssignment(String),

    #[error("invalid block: {0}")]
    InvalidBlock(String),

    #[error("UTXO {0} not found")]
    UtxoNotFound(u64),

    #[error("UTXO {0} already spent")]
    UtxoAlreadySpent(u64),

    #[error("UTXO {0} expired")]
    UtxoExpired(u64),

    #[error("public key already used on this chain")]
    KeyReuse,

    #[error("signature verification failed: {0}")]
    SignatureFailure(String),

    #[error("balance mismatch: givers={givers}, receivers={receivers}, fee={fee}")]
    BalanceMismatch {
        givers: String,
        receivers: String,
        fee: String,
    },

    #[error("deadline expired")]
    DeadlineExpired,

    #[error("agreement refuted")]
    AgreementRefuted,

    #[error("timestamp ordering violation: {0}")]
    TimestampOrder(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("encoding error: {0}")]
    Encoding(String),
}

pub type Result<T> = std::result::Result<T, ChainError>;
