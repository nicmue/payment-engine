pub use error::*;
pub use store::*;

mod error;
mod store;

use rust_decimal::Decimal;
use serde::{Serialize, ser::SerializeStruct};

pub type ClientId = u16;

#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Account {
    client: ClientId,
    available: Decimal,
    held: Decimal,
    locked: bool,
}

impl Account {
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn deposit(&mut self, amount: impl Into<Decimal>) -> AccountResult<()> {
        let amount = amount.into();
        self.available += amount;
        Ok(())
    }

    pub fn withdraw(&mut self, amount: impl Into<Decimal>) -> AccountResult<()> {
        let amount = amount.into();
        if self.locked {
            return Err(AccountError::Locked {
                client: self.client,
            });
        } else if self.available < amount {
            return Err(AccountError::InsufficientAvailable {
                needed: amount,
                available: self.available,
                client: self.client,
            });
        }

        self.available -= amount;
        Ok(())
    }

    pub fn dispute(&mut self, amount: impl Into<Decimal>) -> AccountResult<()> {
        let amount = amount.into();
        self.available -= amount;
        self.held += amount;
        Ok(())
    }

    pub fn release(&mut self, amount: impl Into<Decimal>) -> AccountResult<()> {
        let amount = amount.into();
        if self.held < amount {
            return Err(AccountError::InsufficientHeld {
                needed: amount,
                held: self.held,
                client: self.client,
            });
        }

        self.available += amount;
        self.held -= amount;
        Ok(())
    }

    pub fn chargeback(&mut self, amount: impl Into<Decimal>) -> AccountResult<()> {
        let amount = amount.into();
        if self.held < amount {
            return Err(AccountError::InsufficientHeld {
                needed: amount,
                held: self.held,
                client: self.client,
            });
        }

        self.held -= amount;
        self.locked = true;
        Ok(())
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    #[cfg(test)]
    pub fn create(
        client: u16,
        available: impl Into<Decimal>,
        held: impl Into<Decimal>,
        locked: bool,
    ) -> Self {
        Self {
            client,
            available: available.into(),
            held: held.into(),
            locked,
        }
    }
}

impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // TODO: 4 decimal places precision
        let mut s = serializer.serialize_struct("Account", 5)?;
        s.serialize_field("client", &self.client)?;
        s.serialize_field("available", &self.available)?;
        s.serialize_field("held", &self.held)?;
        s.serialize_field("total", &self.total())?;
        s.serialize_field("locked", &self.locked)?;
        s.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn locked() {
        let mut account = Account::create(1, 10, 0, true);

        assert_eq!(
            account.withdraw(42),
            Err(AccountError::Locked { client: 1 })
        );

        account.deposit(5).unwrap();
        account.dispute(30).unwrap();
        account.release(5).unwrap();
        account.chargeback(10).unwrap();

        assert_eq!(account, Account::create(1, -10, 15, true));
    }

    #[test]
    fn exceed_balance() {
        let mut account = Account::create(1, 10, 20, false);

        assert_eq!(
            account.withdraw(42),
            Err(AccountError::InsufficientAvailable {
                client: 1,
                needed: 42.into(),
                available: 10.into(),
            })
        );
        assert_eq!(
            account.release(42),
            Err(AccountError::InsufficientHeld {
                client: 1,
                needed: 42.into(),
                held: 20.into(),
            })
        );
        assert_eq!(
            account.chargeback(42),
            Err(AccountError::InsufficientHeld {
                client: 1,
                needed: 42.into(),
                held: 20.into(),
            })
        );
    }

    #[test]
    fn payment_flow() {
        let mut account = Account::new(1);

        account.deposit(100).unwrap();
        assert_eq!(account, Account::create(1, 100, 0, false));

        account.withdraw(50).unwrap();
        assert_eq!(account, Account::create(1, 50, 0, false));

        account.dispute(25).unwrap();
        assert_eq!(account, Account::create(1, 25, 25, false));

        account.withdraw(15).unwrap();
        assert_eq!(account, Account::create(1, 10, 25, false));

        assert_eq!(
            account.withdraw(25),
            Err(AccountError::InsufficientAvailable {
                needed: 25.into(),
                available: Decimal::from(10),
                client: 1
            })
        );

        account.release(10).unwrap();
        assert_eq!(account, Account::create(1, 20, 15, false));

        account.deposit(20).unwrap();
        assert_eq!(account, Account::create(1, 40, 15, false));

        account.withdraw(30).unwrap();
        assert_eq!(account, Account::create(1, 10, 15, false));

        account.dispute(20).unwrap();
        assert_eq!(account, Account::create(1, -10, 35, false));

        account.chargeback(5).unwrap();
        assert_eq!(account, Account::create(1, -10, 30, true));

        account.deposit(20).unwrap();
        assert_eq!(account, Account::create(1, 10, 30, true));

        assert_eq!(account.withdraw(5), Err(AccountError::Locked { client: 1 }));

        account.dispute(15).unwrap();
        assert_eq!(account, Account::create(1, -5, 45, true));

        account.release(10).unwrap();
        assert_eq!(account, Account::create(1, 5, 35, true));

        account.release(5).unwrap();
        assert_eq!(account, Account::create(1, 10, 30, true));

        account.chargeback(10).unwrap();
        assert_eq!(account, Account::create(1, 10, 20, true));
    }
}
