use rust_decimal::Decimal;
use thiserror::Error;

use super::ClientId;

pub type AccountResult<T> = Result<T, AccountError>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AccountError {
    #[error(
        "account '{client}' has insuffiecient available funds '{available}', needed: '{needed}'"
    )]
    InsufficientAvailable {
        needed: Decimal,
        available: Decimal,
        client: ClientId,
    },
    #[error("account '{client}' has insuffiecient held funds '{held}', needed: '{needed}'")]
    InsufficientHeld {
        needed: Decimal,
        held: Decimal,
        client: ClientId,
    },
    #[error("account '{client}' locked")]
    Locked { client: ClientId },
}
