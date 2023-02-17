use near_sdk::{
    env::{current_account_id, signer_account_id},
    ext_contract, is_promise_success, Gas, PromiseResult,
};

use crate::{
    ref_finance::{ext_ref_finance, LptId, ShortLiquidityInfo},
    utils::NO_DEPOSIT,
    *,
};

#[ext_contract(ext_self)]
trait ContractCallbackInterface {
    fn get_liquidity_info_callback(&self, lpt_id: LptId, user: Option<AccountId>, order_id: u64);
    fn remove_oldest_liquidity_callback(&mut self, user: Option<AccountId>, order_id: u64);
}

#[near_bindgen]
impl Contract {
    pub fn free_up_liquidity_slot(&mut self, order_id: U128) {
        require!(
            signer_account_id() == self.config.oracle_account_id,
            "You do not have access to call this method."
        );

        let order_id = order_id.0 as u64;

        if let Some((user, order)) = self.get_order_by_id(order_id) {
            match order.status {
                OrderStatus::Pending => {
                    self.get_liquidity_info(order.lpt_id, Some(user), order_id);
                }
                _ => {
                    self.remove_order_by_ids(user, order_id);
                }
            }
        }

        if let Some(take_profit_order) = self.get_take_profit_order_by_id(order_id) {
            match take_profit_order.status {
                OrderStatus::Pending => {
                    self.get_liquidity_info(take_profit_order.lpt_id, None, order_id);
                }
                _ => {
                    self.remove_take_profit_order_by_id(order_id);
                }
            }
        }

        if let Some((pair_id, _)) = self.get_order_per_pair_view_by_id(order_id) {
            self.remove_order_per_pair_view_by_ids(pair_id, order_id);
        }
    }

    #[private]
    pub fn get_liquidity_info(&self, lpt_id: LptId, user: Option<AccountId>, order_id: u64) {
        ext_ref_finance::ext(self.ref_finance_account.clone())
            .with_unused_gas_weight(2)
            .with_attached_deposit(NO_DEPOSIT)
            .get_liquidity(lpt_id.clone())
            .then(
                ext_self::ext(current_account_id())
                    .with_unused_gas_weight(98)
                    .with_attached_deposit(NO_DEPOSIT)
                    .get_liquidity_info_callback(lpt_id, user, order_id),
            );
    }

    #[private]
    pub fn get_liquidity_info_callback(
        &self,
        lpt_id: LptId,
        user: Option<AccountId>,
        order_id: u64,
    ) {
        require!(is_promise_success(), "Some problem with getting liquidity.");

        let liquidity_info: ShortLiquidityInfo = match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(val) => {
                if let Ok(liquidity) = near_sdk::serde_json::from_slice::<ShortLiquidityInfo>(&val)
                {
                    liquidity
                } else {
                    panic!("Some problem with liquidity parsing.")
                }
            }
            PromiseResult::Failed => panic!("Ref finance not found liquidity."),
        };

        let min_amount_x = U128::from(0);
        let min_amount_y = U128::from(0);

        ext_ref_finance::ext(self.ref_finance_account.clone())
            .with_static_gas(Gas::ONE_TERA * 70)
            .with_attached_deposit(NO_DEPOSIT)
            .remove_liquidity(lpt_id, liquidity_info.amount, min_amount_x, min_amount_y)
            .then(
                ext_self::ext(current_account_id())
                    .with_attached_deposit(NO_DEPOSIT)
                    .remove_oldest_liquidity_callback(user, order_id),
            );
    }

    #[private]
    pub fn remove_oldest_liquidity_callback(&mut self, user: Option<AccountId>, order_id: u64) {
        require!(
            is_promise_success(),
            "Some problem with removing liquidity."
        );

        if let Some(account_id) = user {
            self.remove_order_by_ids(account_id, order_id);
        } else {
            self.remove_take_profit_order_by_id(order_id);
        }
    }
}
impl Contract {
    fn get_order_by_id(&self, order_id: u64) -> Option<(AccountId, Order)> {
        for user in self.orders.keys().collect::<Vec<_>>() {
            if let Some(order) = self.orders.get(&user).unwrap().get(&order_id).cloned() {
                return Some((user, order));
            }
        }
        None
    }

    fn get_take_profit_order_by_id(&self, order_id: u64) -> Option<Order> {
        if let Some((_, order)) = self.take_profit_orders.get(&order_id) {
            return Some(order);
        }
        None
    }

    fn get_order_per_pair_view_by_id(&self, order_id: u64) -> Option<(PairId, Order)> {
        for pair_id in self.orders_per_pair_view.keys().collect::<Vec<_>>() {
            if let Some(order) = self
                .orders_per_pair_view
                .get(&pair_id)
                .unwrap()
                .get(&order_id)
                .cloned()
            {
                return Some((pair_id, order));
            }
        }
        None
    }

    fn remove_order_by_ids(&mut self, account_id: AccountId, order_id: u64) {
        let mut orders = self.orders.get(&account_id).unwrap();
        orders.remove(&order_id);
        self.orders.remove(&account_id);
        self.orders.insert(&account_id, &orders);
    }

    fn remove_take_profit_order_by_id(&mut self, order_id: u64) {
        self.take_profit_orders.remove(&order_id);
    }

    fn remove_order_per_pair_view_by_ids(&mut self, pair_id: PairId, order_id: u64) {
        let mut orders = self.orders_per_pair_view.get(&pair_id).unwrap();
        orders.remove(&order_id);
        self.orders_per_pair_view.remove(&pair_id);
        self.orders_per_pair_view.insert(&pair_id, &orders);
    }
}
