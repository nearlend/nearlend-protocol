use crate::*;
use near_sdk::env::block_height;
use std::fmt;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct InterestRateModel {
    pub kink: WRatio,
    pub multiplier_per_block: WRatio,
    pub base_rate_per_block: WRatio,
    pub jump_multiplier_per_block: WRatio,
    pub reserve_factor: WRatio,
    pub rewards_config: Vec<RewardSetting>,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Debug, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RepayInfo {
    pub accrued_interest_per_block: WBalance,
    pub total_amount: WBalance,
}

impl fmt::Display for RepayInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl InterestRateModel {
    pub fn get_kink(&self) -> Ratio {
        Ratio::from(self.kink)
    }

    pub fn get_multiplier_per_block(&self) -> Ratio {
        Ratio::from(self.multiplier_per_block)
    }

    pub fn get_base_rate_per_block(&self) -> Ratio {
        Ratio::from(self.base_rate_per_block)
    }

    pub fn get_jump_multiplier_per_block(&self) -> Ratio {
        Ratio::from(self.jump_multiplier_per_block)
    }

    pub fn get_reserve_factor(&self) -> Ratio {
        Ratio::from(self.reserve_factor)
    }

    pub fn get_rewards_config(&self) -> Vec<RewardSetting> {
        self.rewards_config.clone()
    }

    pub fn set_kink(&mut self, value: WRatio) {
        self.kink = value;
    }

    pub fn set_multiplier_per_block(&mut self, value: WRatio) {
        self.multiplier_per_block = value;
    }

    pub fn set_base_rate_per_block(&mut self, value: WRatio) {
        self.base_rate_per_block = value;
    }

    pub fn set_jump_multiplier_per_block(&mut self, value: WRatio) {
        self.jump_multiplier_per_block = value;
    }

    pub fn set_reserve_factor(&mut self, value: WRatio) {
        self.reserve_factor = value;
    }

    pub fn calculate_accrued_interest(
        &self,
        borrow_rate: Ratio,
        total_borrow: Balance,
        accrued_interest: AccruedInterest,
    ) -> AccruedInterest {
        let current_block_height = block_height();
        let accrued_rate = total_borrow
            * borrow_rate
            * (current_block_height - accrued_interest.last_recalculation_block) as u128
            / RATIO_DECIMALS;

        AccruedInterest {
            accumulated_interest: accrued_interest.accumulated_interest + accrued_rate,
            last_recalculation_block: current_block_height,
        }
    }
}

impl Default for InterestRateModel {
    fn default() -> Self {
        Self {
            kink: WRatio::from(RATIO_DECIMALS),
            base_rate_per_block: WRatio::from(RATIO_DECIMALS),
            multiplier_per_block: WRatio::from(RATIO_DECIMALS),
            jump_multiplier_per_block: WRatio::from(RATIO_DECIMALS),
            reserve_factor: WRatio::from(500),
            rewards_config: Vec::new(),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_accrued_borrow_interest(&self, account: AccountId) -> AccruedInterest {
        self.user_profiles
            .get(&account)
            .unwrap_or_default()
            .borrow_interest
    }

    pub fn get_accrued_supply_interest(&self, account: AccountId) -> AccruedInterest {
        self.user_profiles
            .get(&account)
            .unwrap_or_default()
            .supply_interest
    }

    #[private]
    pub fn set_accrued_supply_interest(
        &mut self,
        account: AccountId,
        accrued_interest: AccruedInterest,
    ) {
        let mut user = self.user_profiles.get(&account).unwrap_or_default();
        user.supply_interest = accrued_interest;
        self.user_profiles.insert(&account, &user);
    }

    #[private]
    pub fn set_accrued_borrow_interest(
        &mut self,
        account: AccountId,
        accrued_interest: AccruedInterest,
    ) {
        let mut user = self.user_profiles.get(&account).unwrap_or_default();
        user.borrow_interest = accrued_interest;
        self.user_profiles.insert(&account, &user);
    }

    #[private]
    pub fn set_rewards_config(&mut self, rewards_config: Vec<RewardSetting>) {
        self.model.rewards_config = rewards_config;
    }
}
