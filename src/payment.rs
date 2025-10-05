pub use error::*;

mod error;
mod processor;

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    thread::JoinHandle,
};

use crossbeam::channel::{self, Sender};

use crate::{account::AccountStore, csv_reader_builder, operation::Operation};

use self::processor::PaymentProcessor;

pub struct PaymentEngine {
    sender: Box<[Sender<Operation>]>,
    processor_handle: Box<[JoinHandle<PaymentResult<AccountStore>>]>,
}

impl PaymentEngine {
    pub fn new(worker: usize) -> Self {
        let (sender, processor_handle): (Vec<_>, Vec<_>) = (0..worker)
            .map(|_| {
                let (sender, receiver) = channel::unbounded();
                let processor = PaymentProcessor::new();

                let handle = std::thread::spawn(move || processor.run(receiver));
                (sender, handle)
            })
            .unzip();

        Self {
            sender: sender.into_boxed_slice(),
            processor_handle: processor_handle.into_boxed_slice(),
        }
    }

    pub fn process_csv<P: AsRef<Path>>(path: P) -> anyhow::Result<AccountStore> {
        let operations = csv_reader_builder()
            .from_path(path)?
            .into_deserialize::<Operation>()
            .filter_map(|res| {
                // we skip lines that can't be deserialized and consider them as wrong
                res.ok()
            });

        let worker = std::thread::available_parallelism()?.get();
        let accounts = PaymentEngine::new(worker).process(operations)?;

        Ok(accounts)
    }

    pub fn process<I>(mut self, operations: I) -> PaymentResult<AccountStore>
    where
        I: IntoIterator<Item = Operation>,
    {
        for operation in operations.into_iter() {
            dispatch_operation(operation, &self.sender)?;
        }

        // dropping all the sender so the receivers will
        // return error and therefore finish the processor loop
        drop(std::mem::take(&mut self.sender));

        let mut accounts = AccountStore::default();
        for handle in std::mem::take(&mut self.processor_handle).into_iter() {
            let store = handle
                .join()
                .map_err(|_| PaymentError::JoiningProcessors)
                .flatten()?;

            accounts.extend(store);
        }

        Ok(accounts)
    }
}

// Operations with the same client id get dispatched to the same processor
// and therefore to the same sender. To achieve this we hash the client id
// and send it to the sender with the same index as the hash modulo the
// number of senders. This is important to avoid races between different
// operations. For example the transaction order of a deposit and withdrawal
// must never change otherwise it could be that we ignore a withdrawal if it
// comes before a deposit that gives us enough credit to cover it. The same
// goes for conflict operations like dispute. It must be ensured that a
// dispute reaches the processor of its client so that the disputed transaction
// is actually present on the processor.
fn dispatch_operation(operation: Operation, sender: &[Sender<Operation>]) -> PaymentResult<()> {
    let client = operation.client();

    let mut hasher = DefaultHasher::new();
    client.hash(&mut hasher);
    let hash = hasher.finish();

    let sender = sender.get((hash % (sender.len() as u64)) as usize).expect(
        "sender should exist as we created the index by modulo the length of the sender array",
    );

    if sender.send(operation).is_err() {
        return Err(PaymentError::DispatchOperation { client });
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use crossbeam::channel::{self, Receiver};
    use itertools::Itertools;
    use rand::seq::SliceRandom;

    use crate::{
        account::ClientId,
        operation::{Conflict, Transaction},
    };

    use super::*;

    #[test]
    fn dispatch() {
        // first create 10 pairs of sender and receiver to which we can dispatch operations
        let (sender, receiver): (Vec<_>, Vec<_>) = (0..10).map(|_| channel::unbounded()).unzip();

        // now create all kinds of operations with a thousand different
        // client and transaction ids, it doesn't matter if the order of the
        // operations makes no sense here as we just want to test that
        // they get dispatched to the correct processor
        let mut operations = (0..1000)
            .flat_map(|i| {
                [
                    (0..10)
                        .map(|j| Operation::from(Transaction::deposit(i, (i as u32) + j, 1)))
                        .collect_vec(),
                    (0..10)
                        .map(|j| Operation::from(Transaction::withdrawal(i, (i as u32) + j, 1)))
                        .collect_vec(),
                    (0..10)
                        .map(|j| Operation::from(Conflict::dispute(i, (i as u32) + j)))
                        .collect_vec(),
                    (0..10)
                        .map(|j| Operation::from(Conflict::resolve(i, (i as u32) + j)))
                        .collect_vec(),
                    (0..10)
                        .map(|j| Operation::from(Conflict::chargeback(i, (i as u32) + j)))
                        .collect_vec(),
                ]
                .into_iter()
                .flatten()
            })
            .collect_vec();

        // shuffle all operations randomly to ensure there is no
        operations.shuffle(&mut rand::rng());

        // now dispatch all operations
        for operation in operations {
            dispatch_operation(operation, &sender).unwrap();
        }

        // drop the sender to ensure the receiver will end after the last dispatched operation
        std::mem::drop(sender);
        // collect all client ids of the received operations into sets for each receiver
        let sets = receiver.into_iter().map(receive_all_clients).collect_vec();

        // check that all sets are distinct, if thats the case two operations
        // for the same client were never sent to different reveicers
        for (i, set) in sets.iter().enumerate() {
            assert!(!set.is_empty());
            for other in sets.iter().skip(i + 1) {
                assert!(set.is_disjoint(other));
            }
        }
    }

    fn receive_all_clients(receiver: Receiver<Operation>) -> HashSet<ClientId> {
        let mut clients = HashSet::new();
        while let Ok(operation) = receiver.recv() {
            clients.insert(operation.client());
        }

        clients
    }
}
