use std::collections::{
    HashMap,
    hash_map::{Entry, VacantEntry},
};

use crate::operation::{Transaction, TransactionError, TransactionId, TransactionResult};

#[derive(Default)]
pub struct TransactionStore(HashMap<TransactionId, TransactionStoreValue>);

#[derive(Debug, PartialEq, Eq)]
pub struct TransactionStoreValue {
    pub transaction: Transaction,
    pub disputed: bool,
}

impl TransactionStore {
    pub fn get_mut(&mut self, id: TransactionId) -> TransactionResult<&mut TransactionStoreValue> {
        self.0.get_mut(&id).ok_or(TransactionError::NotFound { id })
    }

    // currently only used within tests
    #[allow(unused)]
    pub fn insert(&mut self, tx: Transaction) -> TransactionResult<()> {
        self.lock_for_insert(tx)?.finish();
        Ok(())
    }

    pub fn lock_for_insert(&mut self, tx: Transaction) -> TransactionResult<LockForInsert<'_>> {
        match self.0.entry(tx.tx) {
            Entry::Occupied(_) => Err(TransactionError::Conflict { id: tx.tx }),
            Entry::Vacant(vacant) => Ok(LockForInsert(
                vacant,
                TransactionStoreValue {
                    transaction: tx,
                    disputed: false,
                },
            )),
        }
    }
}

#[derive(Debug)]
pub struct LockForInsert<'a>(
    VacantEntry<'a, TransactionId, TransactionStoreValue>,
    TransactionStoreValue,
);

impl LockForInsert<'_> {
    pub fn finish(self) {
        let LockForInsert(entry, tx) = self;
        entry.insert(tx);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_mut_unknown_id() {
        let mut store = TransactionStore::default();

        assert_eq!(store.get_mut(1), Err(TransactionError::NotFound { id: 1 }));
    }

    #[test]
    fn inserting() {
        let mut store = TransactionStore::default();

        store.insert(Transaction::deposit(1, 1, 1)).unwrap();
        assert_eq!(
            store.insert(Transaction::withdrawal(1, 1, 1)),
            Err(TransactionError::Conflict { id: 1 })
        );
        assert_eq!(
            store
                .lock_for_insert(Transaction::withdrawal(1, 1, 1))
                .unwrap_err(),
            TransactionError::Conflict { id: 1 }
        );

        // dont make use of lock so nothing is inserted
        // when it is dropped at the end of the scope
        {
            let _lock = store
                .lock_for_insert(Transaction::deposit(2, 2, 2))
                .unwrap();
        }

        // second attempt can't error but third will
        store
            .lock_for_insert(Transaction::deposit(2, 2, 2))
            .unwrap()
            .finish();
        assert_eq!(
            store
                .lock_for_insert(Transaction::deposit(2, 2, 2))
                .unwrap_err(),
            TransactionError::Conflict { id: 2 }
        );
    }
}
