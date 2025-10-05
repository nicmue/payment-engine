pub use error::*;
pub use transaction_store::*;

mod error;
mod transaction_store;

use derive_more::From;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::account::ClientId;

pub type TransactionId = u32;

#[derive(Deserialize, Debug, PartialEq, Eq, From)]
#[serde(try_from = "OperationDto")]
pub enum Operation {
    Transaction(Transaction),
    Conflict(Conflict),
}

impl Operation {
    pub fn client(&self) -> ClientId {
        match self {
            Operation::Transaction(tx) => tx.client,
            Operation::Conflict(dm) => dm.client,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transaction {
    pub type_: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Decimal,
}

impl Transaction {
    #[allow(unused)]
    pub fn deposit(client: ClientId, tx: TransactionId, amount: impl Into<Decimal>) -> Self {
        Transaction {
            type_: TransactionType::Deposit,
            client,
            tx,
            amount: amount.into(),
        }
    }

    #[allow(unused)]
    pub fn withdrawal(client: ClientId, tx: TransactionId, amount: impl Into<Decimal>) -> Self {
        Transaction {
            type_: TransactionType::Withdrawal,
            client,
            tx,
            amount: amount.into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConflictType {
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Conflict {
    pub type_: ConflictType,
    pub client: ClientId,
    pub tx: TransactionId,
}

impl Conflict {
    #[allow(unused)]
    pub fn dispute(client: ClientId, tx: TransactionId) -> Self {
        Conflict {
            type_: ConflictType::Dispute,
            client,
            tx,
        }
    }

    #[allow(unused)]
    pub fn resolve(client: ClientId, tx: TransactionId) -> Self {
        Conflict {
            type_: ConflictType::Resolve,
            client,
            tx,
        }
    }

    #[allow(unused)]
    pub fn chargeback(client: ClientId, tx: TransactionId) -> Self {
        Conflict {
            type_: ConflictType::Chargeback,
            client,
            tx,
        }
    }
}

#[derive(Deserialize)]
pub struct OperationDto {
    #[serde(rename = "type")]
    type_: String,
    client: ClientId,
    tx: TransactionId,
    amount: Option<Decimal>,
}

impl TryFrom<OperationDto> for Operation {
    type Error = TransactionError;

    fn try_from(dto: OperationDto) -> Result<Self, Self::Error> {
        match dto.type_.as_str() {
            "deposit" => Ok(Operation::Transaction(Transaction {
                type_: TransactionType::Deposit,
                tx: dto.tx,
                client: dto.client,
                amount: dto
                    .amount
                    .ok_or(TransactionError::DeserializeMissingAmount {
                        type_: dto.type_,
                        id: dto.tx,
                    })?,
            })),
            "withdrawal" => Ok(Operation::Transaction(Transaction {
                type_: TransactionType::Withdrawal,
                tx: dto.tx,
                client: dto.client,
                amount: dto
                    .amount
                    .ok_or(TransactionError::DeserializeMissingAmount {
                        type_: dto.type_,
                        id: dto.tx,
                    })?,
            })),
            "dispute" => Ok(Operation::Conflict(Conflict {
                type_: ConflictType::Dispute,
                tx: dto.tx,
                client: dto.client,
            })),
            "resolve" => Ok(Operation::Conflict(Conflict {
                type_: ConflictType::Resolve,
                tx: dto.tx,
                client: dto.client,
            })),
            "chargeback" => Ok(Operation::Conflict(Conflict {
                type_: ConflictType::Chargeback,
                tx: dto.tx,
                client: dto.client,
            })),
            _ => Err(TransactionError::DeserializeUnknownType {
                type_: dto.type_,
                id: dto.tx,
            }),
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use crate::csv_reader_builder;

    use super::*;

    #[test]
    fn from_csv() {
        let data = "\
type, client, tx, amount
deposit, 1, 1, 10
withdrawal, 1, 2, 5
dispute, 1, 1
resolve, 1, 1
chargeback, 1, 1
";

        let operations = csv_reader_builder()
            .from_reader(data.as_bytes())
            .into_deserialize::<Operation>()
            .filter_map(|res| res.ok())
            .collect_vec();

        assert_eq!(
            operations,
            vec![
                Operation::from(Transaction::deposit(1, 1, 10)),
                Operation::from(Transaction::withdrawal(1, 2, 5)),
                Operation::from(Conflict::dispute(1, 1)),
                Operation::from(Conflict::resolve(1, 1)),
                Operation::from(Conflict::chargeback(1, 1)),
            ]
        );
    }
}
