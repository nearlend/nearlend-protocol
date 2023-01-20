extern crate core;

mod big_decimal;
mod cancel_order;
mod common;
mod config;
mod create_order;
mod deposit;
mod execute_order;
mod ft;
mod liquidate_order;
mod market;
mod metadata;
mod oraclehook;
mod price;
mod ref_finance;
mod utils;
mod view;
mod withdraw;

pub use crate::metadata::*;

use crate::big_decimal::*;
use crate::common::PairId;
use crate::config::Config;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, require, AccountId, Balance, PromiseOrValue};
use std::collections::HashMap;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    /// Protocol fee
    protocol_fee: u128,

    /// token ➝ Price
    prices: UnorderedMap<AccountId, Price>,

    /// total orders created on contract
    order_nonce: u64,

    /// user ➝ order_id ➝ Order
    orders: UnorderedMap<AccountId, HashMap<u64, Order>>,

    /// (sell token, buy token) ➝ TradePair
    supported_markets: UnorderedMap<PairId, TradePair>,

    /// User ➝ Token ➝ Balance
    balances: UnorderedMap<AccountId, HashMap<AccountId, Balance>>,

    config: Config,

    /// token id -> market id
    tokens_markets: LookupMap<AccountId, AccountId>,

    /// Protocol profit token_id -> amount
    protocol_profit: LookupMap<AccountId, BigDecimal>,

    /// Ref finance accountId [ as default "dclv2-dev.ref-dev.testnet" ]
    ref_finance_account: AccountId,

    /// Liquidation threshold
    liquidation_threshold: u128,

    /// Volatility rate
    volatility_rate: BigDecimal,

    /// Max value for order amount
    max_order_amount: u128,
}

impl Default for Contract {
    fn default() -> Self {
        env::panic_str("Margin trading contract should be initialized before usage")
    }
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given config. Needs to be called once.
    #[init]
    pub fn new_with_config(owner_id: AccountId, oracle_account_id: AccountId) -> Self {
        Self::new(Config {
            owner_id,
            oracle_account_id,
        })
    }

    #[init]
    #[private]
    pub fn new(config: Config) -> Self {
        require!(!env::state_exists(), "Already initialized");

        Self {
            protocol_fee: 10u128.pow(23),
            prices: UnorderedMap::new(StorageKeys::Prices),
            order_nonce: 0,
            orders: UnorderedMap::new(StorageKeys::Orders),
            supported_markets: UnorderedMap::new(StorageKeys::SupportedMarkets),
            config,
            balances: UnorderedMap::new(StorageKeys::Balances),
            tokens_markets: LookupMap::new(StorageKeys::TokenMarkets),
            protocol_profit: LookupMap::new(StorageKeys::ProtocolProfit),
            ref_finance_account: "dclv2-dev.ref-dev.testnet".parse().unwrap(),
            liquidation_threshold: 10_u128.pow(23),
            volatility_rate: BigDecimal::from(U128(95 * 10_u128.pow(22))),
            max_order_amount: 10_u128.pow(30),
        }
    }

    #[private]
    pub fn set_protocol_fee(&mut self, fee: U128) {
        self.protocol_fee = fee.0
    }

    #[private]
    pub fn add_token_market(&mut self, token_id: AccountId, market_id: AccountId) {
        self.tokens_markets.insert(&token_id, &market_id);
    }

    #[private]
    pub fn set_liquidation_threshold(&mut self, threshold: U128) {
        self.liquidation_threshold = threshold.0;
    }

    #[private]
    pub fn set_volatility_rate(&mut self, rate: U128) {
        self.volatility_rate = BigDecimal::from(rate)
    }

    #[private]
    pub fn set_max_order_amount(&mut self, value: U128) {
        self.max_order_amount = value.0
    }

    #[private]
    pub fn set_max_leverage(&mut self, pair: &PairId, leverage: U128) {
        let mut traid_pair = self
            .supported_markets
            .get(pair)
            .unwrap_or_else(|| panic!("Max leverage for pair {} | {} not found", pair.0, pair.1));

        traid_pair.max_leverage = leverage;
        self.supported_markets.insert(pair, &traid_pair);
    }

    pub fn get_max_leverage(&self, pair: &PairId) -> U128 {
        self.supported_markets
            .get(pair)
            .unwrap_or_else(|| panic!("Max leverage for pair {} | {} not found", pair.0, pair.1))
            .max_leverage
    }
}

impl Contract {
    pub fn get_swap_fee(&self, order: &Order) -> U128 {
        let pair = (order.sell_token.clone(), order.buy_token.clone());
        self.supported_markets
            .get(&pair)
            .unwrap_or_else(|| panic!("Swap fee for pair {} | {} not found", pair.0, pair.1))
            .swap_fee
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_get_max_leverage() {
        let mut contract = Contract::new_with_config(
            "owner_id.testnet".parse().unwrap(),
            "oracle_account_id.testnet".parse().unwrap(),
        );
        let pair = (
            AccountId::from_str("usdt.fakes.testnet").unwrap(),
            AccountId::from_str("wrap.testnet").unwrap(),
        );

        let pair_data = TradePair {
            sell_ticker_id: "USDt".to_string(),
            sell_token: "usdt.fakes.testnet".parse().unwrap(),
            sell_token_decimals: 24,
            sell_token_market: "usdt_market.develop.v1.omomo-finance.testnet"
                .parse()
                .unwrap(),
            buy_ticker_id: "near".to_string(),
            buy_token: "wrap.testnet".parse().unwrap(),
            buy_token_decimals: 24,
            pool_id: "usdt.fakes.testnet|wrap.testnet|2000".to_string(),
            max_leverage: U128(25 * 10_u128.pow(23)),
            swap_fee: U128(10u128.pow(20)),
        };
        contract.add_pair(pair_data.clone());

        let result = pair_data.max_leverage;
        let max_leverage = contract.get_max_leverage(&pair);
        assert_eq!(max_leverage, result);
    }

    #[test]
    fn test_set_max_leverage() {
        let mut contract = Contract::new_with_config(
            "owner_id.testnet".parse().unwrap(),
            "oracle_account_id.testnet".parse().unwrap(),
        );
        let pair = (
            AccountId::from_str("usdt.fakes.testnet").unwrap(),
            AccountId::from_str("wrap.testnet").unwrap(),
        );

        let pair_data = TradePair {
            sell_ticker_id: "USDt".to_string(),
            sell_token: "usdt.fakes.testnet".parse().unwrap(),
            sell_token_decimals: 24,
            sell_token_market: "usdt_market.develop.v1.omomo-finance.testnet"
                .parse()
                .unwrap(),
            buy_ticker_id: "near".to_string(),
            buy_token: "wrap.testnet".parse().unwrap(),
            buy_token_decimals: 24,
            pool_id: "usdt.fakes.testnet|wrap.testnet|2000".to_string(),
            max_leverage: U128(25 * 10_u128.pow(23)),
            swap_fee: U128(10u128.pow(20)),
        };
        contract.add_pair(pair_data);

        contract.set_max_leverage(&pair, U128(10 * 10_u128.pow(24)));
        let max_leverage = contract.get_max_leverage(&pair);
        assert_eq!(max_leverage, U128(10 * 10_u128.pow(24)));
    }

    #[test]
    fn test_get_swap_fee() {
        let mut contract = Contract::new_with_config(
            "owner_id.testnet".parse().unwrap(),
            "oracle_account_id.testnet".parse().unwrap(),
        );

        let pair_data = TradePair {
            sell_ticker_id: "USDt".to_string(),
            sell_token: "usdt.fakes.testnet".parse().unwrap(),
            sell_token_decimals: 24,
            sell_token_market: "usdt_market.develop.v1.omomo-finance.testnet"
                .parse()
                .unwrap(),
            buy_ticker_id: "near".to_string(),
            buy_token: "wrap.testnet".parse().unwrap(),
            buy_token_decimals: 24,
            pool_id: "usdt.fakes.testnet|wrap.testnet|2000".to_string(),
            max_leverage: U128(25 * 10_u128.pow(23)),
            swap_fee: U128(3 * 10u128.pow(20)),
        };
        contract.add_pair(pair_data.clone());

        let order = Order {
            status: OrderStatus::Pending,
            order_type: OrderType::Buy,
            amount: 1000000000000000000000000000,
            sell_token: "usdt.fakes.testnet".parse().unwrap(),
            buy_token: "wrap.testnet".parse().unwrap(),
            leverage: BigDecimal::from(1.0),
            sell_token_price: Price {
                ticker_id: "USDT".to_string(),
                value: BigDecimal::from(1.01),
            },
            buy_token_price: Price {
                ticker_id: "near".to_string(),
                value: BigDecimal::from(3.07),
            },
            block: 105210654,
            lpt_id: "usdt.fakes.testnet|wrap.testnet|2000#238".to_string(),
        };

        let swap_fee = contract.get_swap_fee(&order);

        assert_eq!(swap_fee, pair_data.swap_fee);
    }
}
