use crate::*;

impl Contract {

    pub fn get_controller_address(&self) -> AccountId {
        let config: Config = self.get_contract_config();

        return AccountId::new_unchecked(config.controller_account_id.to_string());
    }

    pub fn get_contract_address(&self) -> AccountId {
        return env::current_account_id().clone();
    }

    pub fn get_underlying_contract_address(&self) -> AccountId {
        return self.underlying_token.clone();
    }

    pub fn get_exchange_rate(&self, underlying_balance: WBalance) -> Balance {
        if self.token.total_supply == 0 {
            return self.initial_exchange_rate;
        }
        return (Balance::from(underlying_balance) + self.total_borrows - self.total_supplies)
            / self.token.total_supply;
    }
}

#[near_bindgen]
impl Contract {


    pub fn get_total_supplies(&self) -> Balance {
        return self.total_supplies;
    }

    pub fn get_total_borrows(&self) -> Balance {
        return self.total_borrows;
    }


    #[private]
    pub fn set_total_supplies(&mut self, amount: Balance) -> Balance {
        self.total_supplies = amount;
        return self.get_total_supplies();
    }

    #[private]
    pub fn set_total_borrows(&mut self, amount: Balance) -> Balance {
        self.total_borrows = amount;
        return self.get_total_borrows();
    }

    pub fn mint(&mut self, account_id: &AccountId, amount: WBalance) {
        if !self.token.accounts.contains_key(&account_id.clone()) {
            self.token.internal_register_account(&account_id.clone());
        }
        self.token.internal_deposit(&account_id, amount.into());
    }

    pub fn burn(&mut self, account_id: &AccountId, amount: WBalance) {
        if !self.token.accounts.contains_key(&account_id.clone()) {
            panic!("User with account {} wasn't found", account_id.clone().to_string());
        }
        self.token.internal_withdraw(&account_id, amount.into());
    }
}
