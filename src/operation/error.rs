use thiserror::Error;

use super::TransactionId;

pub type TransactionResult<T> = Result<T, TransactionError>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TransactionError {
    #[error("transaction '{id}' not found")]
    NotFound { id: TransactionId },
    #[error("transaction '{id}' with different action already exists")]
    Conflict { id: TransactionId },
    #[error("failed to deserialize transaction '{id}' of type '{type_}': missing amount")]
    DeserializeMissingAmount { type_: String, id: TransactionId },
    #[error("failed to deserialize transaction '{id}': unknown type '{type_}'")]
    DeserializeUnknownType { type_: String, id: TransactionId },
}
