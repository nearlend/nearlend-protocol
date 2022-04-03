use crate::*;

const GAS_FOR_REPAY: Gas = Gas(95_000_000_000_000);

impl Contract {
    
    pub fn repay(&mut self, token_amount: WBalance) -> PromiseOrValue<U128> {
        require!(env::prepaid_gas() >= GAS_FOR_REPAY, "Prepaid gas is not enough for repay flow");
        self.mutex_account_lock(String::from("repay"));

        underlying_token::ft_balance_of(
            self.get_contract_address(),
            self.get_underlying_contract_address(),
            NO_DEPOSIT,
            TGAS,
        )
        .then(ext_self::repay_balance_of_callback(
            token_amount,
            env::current_account_id(),
            NO_DEPOSIT,
            self.terra_gas(40),
        ))
        .into()
    }
}

#[near_bindgen]
impl Contract {

    #[private]
    pub fn repay_balance_of_callback(&mut self, token_amount: WBalance) -> PromiseOrValue<U128> {
        if !is_promise_success() {
            log!("{}", Events::RepayFailedToGetUnderlyingBalance(env::signer_account_id(), Balance::from(token_amount), self.get_contract_address(), self.get_underlying_contract_address()));
            self.mutex_account_unlock();
            return PromiseOrValue::Value(token_amount); 
        }

        let balance_of: Balance = match env::promise_result(0) {
            PromiseResult::NotReady => 0,
            PromiseResult::Failed => 0,
            PromiseResult::Successful(result) => near_sdk::serde_json::from_slice::<U128>(&result)
                .unwrap()
                .into(),
        };

        let borrow_rate: Balance = self.get_borrow_rate(
            U128(balance_of - Balance::from(token_amount)),
            U128(self.get_total_borrows()),
            U128(self.total_reserves),
        );
        let borrow_amount = self.get_account_borrows(env::signer_account_id());
        let borrow_accrued_interest = self.model.calculate_accrued_interest(
            borrow_rate, 
            self.get_account_borrows(env::signer_account_id()), 
            self.get_accrued_borrow_interest(env::signer_account_id())
        );
        let borrow_with_rate_amount = borrow_amount + borrow_accrued_interest.accumulated_interest;
        self.set_accrued_borrow_interest(env::signer_account_id(), borrow_accrued_interest);

        require!(Balance::from(token_amount) >= borrow_with_rate_amount, format!("repay amount {} is less than actual debt {}", Balance::from(token_amount), borrow_with_rate_amount));

        controller::repay_borrows(
            env::signer_account_id(),
            self.get_contract_address(),
            U128(borrow_amount),
            self.get_controller_address(),
            NO_DEPOSIT,
            self.terra_gas(5),
        )
        .then(ext_self::controller_repay_borrows_callback(
            token_amount,
            U128(borrow_with_rate_amount),
            env::current_account_id(),
            NO_DEPOSIT,
            self.terra_gas(5),
        ))
        .into()
    }

    #[private]
    pub fn controller_repay_borrows_callback(
        &mut self,
        amount: WBalance,
        borrow_amount: WBalance,
    ) -> PromiseOrValue<U128> {
        if !is_promise_success() {
            log!("{}", Events::RepayFailedToUpdateUserBalance(env::signer_account_id(), Balance::from(borrow_amount)));
            self.mutex_account_unlock();
            return PromiseOrValue::Value(amount);
        }

        let extra_balance = Balance::from(amount) - Balance::from(borrow_amount);
        self.decrease_borrows(
            env::signer_account_id(),
            U128(self.get_account_borrows(env::signer_account_id())),
        );
        self.set_accrued_borrow_interest(env::signer_account_id(), AccruedInterest::default());

        self.mutex_account_unlock();
        log!("{}", Events::RepaySuccess(env::signer_account_id(), Balance::from(borrow_amount)));
        PromiseOrValue::Value(U128(extra_balance))
    }
}
