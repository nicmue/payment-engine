use thiserror::Error;

use crate::{
    account::{AccountError, ClientId},
    operation::{TransactionError, TransactionId},
};

pub type PaymentResult<T> = Result<T, PaymentError>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PaymentError {
    #[error("transaction '{id}' already disputed")]
    TransactionAlreadyDisputed { id: TransactionId },
    #[error("transaction '{id}' not disputed")]
    TransactionNotDisputed { id: TransactionId },
    #[error(
        "dispute operation for transaction '{tx}' has a client mismatch, expected: '{expected}', actual: '{actual}'"
    )]
    ConflictClientMismatch {
        tx: TransactionId,
        expected: ClientId,
        actual: ClientId,
    },
    #[error("transaction '{tx}' cannot be disputed because its a withdrawal ")]
    WithdrawalCannotBeDisputed { tx: TransactionId },

    #[error("deposit failed")]
    Deposit(#[source] AccountError),
    #[error("withdrawal failed")]
    Withdrawal(#[source] AccountError),
    #[error("hold failed")]
    Hold(#[source] AccountError),
    #[error("release failed")]
    Release(#[source] AccountError),
    #[error("chargeback failed")]
    Chargeback(#[source] AccountError),

    #[error(transparent)]
    Transaction(#[from] TransactionError),

    #[error("failed to dispatch opration for client '{client}'")]
    DispatchOperation { client: ClientId },
    #[error("failed to join payment processors")]
    JoiningProcessors,
}
