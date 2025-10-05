# Payment Engine

This is a little fun project to simulate a payment engine. The payment engine takes a `csv` as input. This csv describes transactions/operations that are performed for clients and their accounts:

```csv
type, client, tx, amount
deposit, 1, 1, 42.0
withdrawal, 2, 2, 10
deposit, 2, 3, 10
withdrawal, 1, 4, 10.5
withdrawal, 2, 5, 6.75
dispute, 1, 1
```

After processing all operations the payment engine will print the state of all client accounts to stdout. Given the example above this will result in:

```csv
client,available,held,total,locked
1,-10.5,42,31.5,false
2,3.25,0,3.25,false
```

To run the payment engine from the command line simply run:

```bash
$ cargo run -- operations.csv > client-accounts.csv
```

## Operations

There are two kinds of operations. Transactions and conflicts. Each operation consists of the columns `type`, `client`, `tx` and `amount` whereby `amount` is only mandatory and used for transaction operations

### Transactions

- `deposit`<br/>
  A `deposit` increases the available credit of the specified clients account.
- `withdrawal`<br/>
  A `withdrawal` decreases the available credit of the specified clients account. If there is not enough available credit the `withdrawal` is ignored.

### Conflicts

- `dispute`<br/>
  A `dispute` marks a previous issued `deposit` as disputed and moves the amount of the referenced `deposit` from available to held credit. Only `deposit` transactions can be disputed, `withdrawal` transactions can't be disputed. A `dispute` is never ignored. Even if an account doesn't have enough available credit the `dispute` is still issued. This can lead to a negative available credit.
- `resolve`<br/>
  A `resolve` marks a previous disputed `deposit` as resolved and moves the previously held credit back to the clients available credit. If the `resolve` exceeds the currently held balance it is ignored.
- `chargeback`<br/>
  A `chargeback` withdraws the previously held credit of a disputed `deposit`. This means the held credit is decreased by the disputed amount. Furthermore an account that experienced a `chargeback` is marked as `locked`. If the `chargeback` exceeds the currently held credit it is ignored.

If any of the above operations are issued for an unknown transaction or a transaction was already disputed (for `dispute`) or is currently not marked for dispute (for `resolve` and `chargeback`) the operation is ignored.

## Locked accounts

Locked accounts are explicitly marked by the `locked` column and can no longer `withdraw` any credit even if they have enough available. However `deposit`, `dispute`, `resolve` and `chargeback` operations are still allowed. The rational is that a client should no longer be allowed to `withdraw` credit from its account but operations like `dispute` are technical operations and are normally issued by a partner payment provider. Also other systems can read the flag and decide what further actions the client can do with its account.

Its of course up for discussion if a `chargeback` should still be doable if an account is `locked`. A malicious client together with a collaborating payment provider could just chargeback all deposits instead of withdrawing them. In this implementation we assume that `chargeback` operations are validated by a friendly payment provider and therefore they are still allowed on `locked` accounts.

## Architecture

To improve the processing time of big inputs, the given operations are executed in parallel. To achieve this the actual `PaymentEngine` spawns `n` worker `PaymentProcessor` threads. By default `n` is the number of available operating system threads (see [1] for more information). Each of these spawned `PaymentProcessor` threads is responsible for a set of clients. Meaning all transactions of a client will end up on the same `PaymentProcessor`. This is needed to ensure the order of operations. Take the following example:

```csv
type, client, tx, amount
deposit, 1, 1, 10
withdrawal, 1, 2, 5
```

The `deposit` and `withdrawal` indirectly depend on each other. You can't withdraw if you have never deposited before. Therefore the order of operations and with it the order of transaction ids must be preserved. The same goes for conflict operations. A `dispute` would be ignored if the referenced `deposit` is only processed afterwards, just because of a broken transaction ordering.

To ensure fair work distribution over all `PaymentProcessor` threads, the operations are dispatched to each `PaymentProcessor` by hashing its corresponding client id. This ensures all threads will receive operations to work on and operations for the same client will be processed by the same thread and `PaymentProcessor`.

[1] https://doc.rust-lang.org/std/thread/fn.available_parallelism.html

## Testing

Each component contains unit tests if applicable. Integration tests can be found under the `tests` folder. Each directory under `tests/test_cases` contains a pair of `input.csv` and `output.csv` which resemble the wanted input output combination. The test implementation itself can be found in `tests/interation_tests.rs`. To run all tests simply run:

```bash
$ cargo test
```
