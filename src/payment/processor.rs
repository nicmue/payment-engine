use crossbeam::channel::Receiver;

use crate::{
    account::AccountStore,
    operation::{
        Conflict, ConflictType, Operation, Transaction, TransactionStore, TransactionType,
    },
};

use super::{PaymentError, PaymentResult};

#[derive(Default)]
pub struct PaymentProcessor {
    accounts: AccountStore,
    transactions: TransactionStore,
}

impl PaymentProcessor {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn run(mut self, receiver: Receiver<Operation>) -> PaymentResult<AccountStore> {
        while let Ok(operation) = receiver.recv() {
            if self.process(operation).is_err() {
                // The current processing scheme is designed to
                // ignore errors and continue processing.
            }
        }

        Ok(self.accounts)
    }

    #[allow(unused)]
    pub fn accounts(&self) -> &AccountStore {
        &self.accounts
    }

    fn process(&mut self, operation: Operation) -> PaymentResult<()> {
        match operation {
            Operation::Transaction(tx) => self.transaction(tx),
            Operation::Conflict(dispute) => self.conflict(dispute),
        }
    }

    fn transaction(&mut self, tx: Transaction) -> PaymentResult<()> {
        // We first lock the slot for the transaction in the trasnaction
        // store to ensure there is not already a transaction with the
        // same id present.
        let lock = self.transactions.lock_for_insert(tx)?;
        match tx.type_ {
            TransactionType::Deposit => self
                .accounts
                .get_mut(tx.client)
                .deposit(tx.amount)
                .map_err(PaymentError::Deposit)?,
            TransactionType::Withdrawal => self
                .accounts
                .get_mut(tx.client)
                .withdraw(tx.amount)
                .map_err(PaymentError::Withdrawal)?,
        }

        lock.finish();
        Ok(())
    }

    fn conflict(&mut self, conflict: Conflict) -> PaymentResult<()> {
        let target = self.transactions.get_mut(conflict.tx)?;

        let tx = target.transaction.tx;
        let client = target.transaction.client;
        let amount = target.transaction.amount;

        if client != conflict.client {
            return Err(PaymentError::ConflictClientMismatch {
                tx,
                expected: client,
                actual: conflict.client,
            });
        } else if matches!(target.transaction.type_, TransactionType::Withdrawal) {
            return Err(PaymentError::WithdrawalCannotBeDisputed { tx });
        }

        match conflict.type_ {
            ConflictType::Dispute => {
                if target.disputed {
                    return Err(PaymentError::TransactionAlreadyDisputed { id: tx });
                }

                self.accounts
                    .get_mut(client)
                    .dispute(amount)
                    .map_err(PaymentError::Hold)?;
                target.disputed = true;
            }
            ConflictType::Resolve => {
                if !target.disputed {
                    return Err(PaymentError::TransactionNotDisputed { id: tx });
                }

                self.accounts
                    .get_mut(client)
                    .release(amount)
                    .map_err(PaymentError::Release)?;
                target.disputed = false;
            }
            ConflictType::Chargeback => {
                if !target.disputed {
                    return Err(PaymentError::TransactionNotDisputed { id: tx });
                }

                self.accounts
                    .get_mut(client)
                    .chargeback(amount)
                    .map_err(PaymentError::Chargeback)?;
                target.disputed = false;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use crate::{
        account::{Account, AccountError},
        operation::TransactionError,
    };

    use super::*;

    #[test]
    fn conflict_client_mismatch() {
        let mut p = PaymentProcessor::new();
        p.transaction(Transaction::deposit(1, 1, 1)).unwrap();

        assert_eq!(
            p.conflict(Conflict::dispute(2, 1)),
            Err(PaymentError::ConflictClientMismatch {
                tx: 1,
                expected: 1,
                actual: 2
            })
        );
    }

    #[test]
    fn withdrawal_cannot_be_disputed() {
        let mut p = PaymentProcessor::new();
        p.transaction(Transaction::deposit(1, 1, 1)).unwrap();
        p.transaction(Transaction::withdrawal(1, 2, 1)).unwrap();

        assert_eq!(
            p.conflict(Conflict::dispute(1, 2)),
            Err(PaymentError::WithdrawalCannotBeDisputed { tx: 2 })
        )
    }

    #[test]
    fn tx_already_disputed() {
        let mut p = PaymentProcessor::new();
        p.transaction(Transaction::deposit(1, 1, 1)).unwrap();
        p.conflict(Conflict::dispute(1, 1)).unwrap();

        assert_eq!(
            p.conflict(Conflict::dispute(1, 1)),
            Err(PaymentError::TransactionAlreadyDisputed { id: 1 })
        );
    }

    #[test]
    fn tx_not_disputed() {
        let mut p = PaymentProcessor::new();
        p.transaction(Transaction::deposit(1, 1, 1)).unwrap();

        assert_eq!(
            p.conflict(Conflict::resolve(1, 1)),
            Err(PaymentError::TransactionNotDisputed { id: 1 })
        );
        assert_eq!(
            p.conflict(Conflict::chargeback(1, 1)),
            Err(PaymentError::TransactionNotDisputed { id: 1 })
        );
    }

    #[test]
    fn payment_flow() {
        let mut p = PaymentProcessor::new();

        p.transaction(Transaction::deposit(1, 1, 10)).unwrap();
        p.transaction(Transaction::deposit(1, 2, 20)).unwrap();
        p.transaction(Transaction::withdrawal(1, 3, 10)).unwrap();
        // Account { client: 1, available: 20, held: 0, locked: false }
        // Account { client: 2, available: 0, held: 0, locked: false }

        assert_eq!(
            p.transaction(Transaction::deposit(2, 2, 20)),
            Err(PaymentError::Transaction(TransactionError::Conflict {
                id: 2
            }))
        );

        p.transaction(Transaction::deposit(2, 4, 20)).unwrap();
        // Account { client: 1, available: 20, held: 0, locked: false }
        // Account { client: 2, available: 20, held: 0, locked: false }
        p.conflict(Conflict::dispute(1, 1)).unwrap();
        // Account { client: 1, available: 10, held: 10, locked: false }
        // Account { client: 2, available: 20, held: 0, locked: false }

        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, 10, 10, false),
                Account::create(2, 20, 0, false),
            ]
        );

        assert_eq!(
            p.conflict(Conflict::dispute(2, 2)),
            Err(PaymentError::ConflictClientMismatch {
                tx: 2,
                expected: 1,
                actual: 2
            })
        );
        assert_eq!(
            p.conflict(Conflict::resolve(2, 4)),
            Err(PaymentError::TransactionNotDisputed { id: 4 })
        );

        p.conflict(Conflict::dispute(2, 4)).unwrap();
        // Account { client: 1, available: 10, held: 10, locked: false }
        // Account { client: 2, available: 0, held: 20, locked: false }

        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, 10, 10, false),
                Account::create(2, 0, 20, false),
            ]
        );

        p.conflict(Conflict::dispute(1, 2)).unwrap();
        // Account { client: 1, available: -10, held: 30, locked: false }
        // Account { client: 2, available: 0, held: 20, locked: false }

        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, -10, 30, false),
                Account::create(2, 0, 20, false),
            ]
        );

        assert_eq!(
            p.conflict(Conflict::chargeback(1, 3)),
            Err(PaymentError::WithdrawalCannotBeDisputed { tx: 3 })
        );

        p.conflict(Conflict::chargeback(1, 1)).unwrap();
        // Account { client: 1, available: -10, held: 20, locked: true }
        // Account { client: 2, available: 0, held: 20, locked: false }

        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, -10, 20, true),
                Account::create(2, 0, 20, false),
            ]
        );

        // withdrawal is now locked for client 1
        assert_eq!(
            p.transaction(Transaction::withdrawal(1, 5, 15)),
            Err(PaymentError::Withdrawal(AccountError::Locked { client: 1 }))
        );

        // but he can still deposit to level out debt
        p.transaction(Transaction::deposit(1, 5, 10)).unwrap();
        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, 0, 20, true),
                Account::create(2, 0, 20, false),
            ]
        );

        // disputes can still happen though
        p.conflict(Conflict::resolve(1, 2)).unwrap();
        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, 20, 0, true),
                Account::create(2, 0, 20, false),
            ]
        );

        p.conflict(Conflict::dispute(1, 2)).unwrap();
        // Account { client: 1, available: 0, held: 20, locked: true }
        // Account { client: 2, available: 0, held: 20, locked: false }
        p.conflict(Conflict::chargeback(1, 2)).unwrap();
        // Account { client: 1, available: 0, held: 0, locked: true }
        // Account { client: 2, available: 0, held: 20, locked: false }

        assert_eq!(
            sorted_accounts(p.accounts()),
            vec![
                Account::create(1, 0, 0, true),
                Account::create(2, 0, 20, false),
            ]
        );
    }

    fn sorted_accounts(accounts: &AccountStore) -> Vec<Account> {
        accounts
            .into_iter()
            .map(|(client, acc)| (*client, acc.clone()))
            .sorted_by_key(|(client, _)| *client)
            .map(|(_, acc)| acc)
            .collect_vec()
    }
}
