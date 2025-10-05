use std::collections::HashMap;

use derive_more::IntoIterator;

use super::{Account, ClientId};

#[derive(Default, Debug, IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct AccountStore(HashMap<ClientId, Account>);

impl AccountStore {
    pub fn get_mut(&mut self, client: ClientId) -> &mut Account {
        self.0.entry(client).or_insert_with(|| Account::new(client))
    }

    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }
}
