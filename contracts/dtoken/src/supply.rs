use crate::*;
use general::ratio::{BigBalance, Ratio};

const GAS_FOR_SUPPLY: Gas = Gas(120_000_000_000_000);

impl Contract {
    pub fn supply(&mut self, token_amount: WBalance) -> PromiseOrValue<WBalance> {
        require!(
            env::prepaid_gas() >= GAS_FOR_SUPPLY,
            "Prepaid gas is not enough for supply flow"
        );
        self.mutex_account_lock(Actions::Supply, token_amount, self.terra_gas(120))
    }
    pub fn post_supply(&mut self, token_amount: WBalance) -> PromiseOrValue<WBalance> {
        if !is_promise_success() {
            return PromiseOrValue::Value(token_amount);
        }

        underlying_token::ft_balance_of(
            env::current_account_id(),
            self.get_underlying_contract_address(),
            NO_DEPOSIT,
            TGAS,
        )
        .then(ext_self::supply_balance_of_callback(
            token_amount,
            env::current_account_id(),
            NO_DEPOSIT,
            self.terra_gas(50),
        ))
        .into()
    }

    pub fn get_supplies_by_account(&self, account: AccountId) -> Balance {
        self.get_account_supplies(account)
    }

    pub fn decrease_supplies(&mut self, account: AccountId, token_amount: WBalance) -> Balance {
        let supplies = self.get_account_supplies(account.clone());
        let new_supplies = supplies - Balance::from(token_amount);

        self.set_account_supplies(account, U128(new_supplies))
    }

    pub fn increase_supplies(&mut self, account: AccountId, token_amount: WBalance) -> Balance {
        let supplies: Balance = self.get_account_supplies(account.clone());
        let new_supplies = supplies + Balance::from(token_amount);

        self.set_account_supplies(account, U128(new_supplies))
    }

    pub fn set_account_supplies(&mut self, account: AccountId, token_amount: WBalance) -> Balance {
        let mut user = self.user_profiles.get(&account).unwrap_or_default();
        user.supplies = Balance::from(token_amount);
        self.user_profiles.insert(&account, &user);

        self.get_account_supplies(account)
    }

    pub fn get_account_supplies(&self, account: AccountId) -> Balance {
        self.user_profiles
            .get(&account)
            .unwrap_or_default()
            .supplies
    }
}

#[near_bindgen]
impl Contract {
    #[allow(dead_code)]
    #[private]
    pub fn supply_balance_of_callback(
        &mut self,
        token_amount: WBalance,
    ) -> PromiseOrValue<WBalance> {
        if !is_promise_success() {
            log!(
                "{}",
                Events::SupplyFailedToGetUnderlyingBalance(
                    env::signer_account_id(),
                    Balance::from(token_amount),
                    self.get_contract_address(),
                    self.get_underlying_contract_address()
                )
            );
            self.mutex_account_unlock();
            return PromiseOrValue::Value(token_amount);
        }

        let balance_of: Balance = match env::promise_result(0) {
            PromiseResult::NotReady => 0,
            PromiseResult::Failed => 0,
            PromiseResult::Successful(result) => {
                near_sdk::serde_json::from_slice::<WBalance>(&result)
                    .unwrap()
                    .into()
            }
        };

        let exchange_rate =
            self.get_exchange_rate((balance_of - Balance::from(token_amount)).into());
        let dtoken_amount =
            WBalance::from((BigBalance::from(token_amount.0) / exchange_rate).round_u128());

        let interest_rate_model = self.config.get().unwrap().interest_rate_model;
        let supply_rate: Ratio = self.get_supply_rate(
            U128(balance_of - Balance::from(token_amount)),
            U128(self.get_total_borrows()),
            U128(self.get_total_reserves()),
            interest_rate_model.get_reserve_factor(),
        );
        let accrued_interest = self.get_accrued_supply_interest(env::signer_account_id());
        let accrued_supply_interest = interest_rate_model.calculate_accrued_interest(
            supply_rate,
            self.get_supplies_by_account(env::signer_account_id()),
            accrued_interest,
        );
        self.set_accrued_supply_interest(env::signer_account_id(), accrued_supply_interest);

        // Dtokens minting and adding them to the user account
        self.mint(self.get_signer_address(), dtoken_amount);
        self.increase_supplies(self.get_signer_address(), token_amount);

        log!(
            "Supply from Account {} to Dtoken contract {} with tokens amount {} was successfully done!",
            self.get_signer_address(),
            self.get_contract_address(),
            Balance::from(token_amount)
        );

        controller::increase_supplies(
            env::signer_account_id(),
            self.get_contract_address(),
            token_amount,
            self.get_controller_address(),
            NO_DEPOSIT,
            self.terra_gas(5),
        )
        .then(ext_self::controller_increase_supplies_callback(
            token_amount,
            dtoken_amount,
            env::current_account_id(),
            NO_DEPOSIT,
            self.terra_gas(20),
        ))
        .into()
    }

    #[allow(dead_code)]
    #[private]
    pub fn controller_increase_supplies_callback(
        &mut self,
        amount: WBalance,
        dtoken_amount: WBalance,
    ) -> PromiseOrValue<U128> {
        if !is_promise_success() {
            log!(
                "{}",
                Events::SupplyFailedToIncreaseSupplyOnController(
                    env::signer_account_id(),
                    Balance::from(amount)
                )
            );
            self.burn(&self.get_signer_address(), dtoken_amount);
            self.decrease_supplies(self.get_signer_address(), amount);

            self.mutex_account_unlock();
            return PromiseOrValue::Value(amount);
        }
        log!(
            "{}",
            Events::SupplySuccess(env::signer_account_id(), Balance::from(amount))
        );
        self.mutex_account_unlock();
        PromiseOrValue::Value(U128(0))
    }
}
