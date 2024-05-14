use anchor_lang::prelude::*;
use derivative::Derivative;

use crate::consts::{
    CLOSE_TO_INSOLVENCY_RISKY_LTV, GLOBAL_ALLOWED_BORROW_VALUE, GLOBAL_UNHEALTHY_BORROW_VALUE,
    LIQUIDATION_CLOSE_FACTOR, LIQUIDATION_CLOSE_VALUE, MAX_LIQUIDATABLE_VALUE_AT_ONCE,
    PROGRAM_VERSION,
};

static_assertions::const_assert_eq!(0, std::mem::size_of::<LendingMarket>() % 8);

#[derive(PartialEq, Eq, Derivative)]
#[account(zero_copy)]
#[repr(C)]
pub struct LendingMarket {
    pub version: u64,
    pub bump_seed: u64,

    pub owner: Pubkey,

    pub owner_cached: Pubkey,

    pub quote_currency: [u8; 32],

    pub emergency_mode: u8,

    pub autodeleverage_enabled: u8,

    pub borrow_disabled: u8,

    pub price_refresh_trigger_to_max_age_pct: u8,
    pub liquidation_max_debt_close_factor_pct: u8,
    pub insolvency_risk_unhealthy_ltv_pct: u8,
    pub padding1: [u8; 2],

    pub min_full_liquidation_value_threshold: u64,

    pub max_liquidatable_debt_market_value_at_once: u64,
    pub global_unhealthy_borrow_value: u64,
    pub global_allowed_borrow_value: u64,
}

impl Default for LendingMarket {
    fn default() -> Self {
        Self {
            version: 0,
            bump_seed: 0,
            owner: Pubkey::default(),
            owner_cached: Pubkey::default(),
            quote_currency: [0; 32],
            emergency_mode: 0,
            autodeleverage_enabled: 0,
            borrow_disabled: 0,
            price_refresh_trigger_to_max_age_pct: 0,
            liquidation_max_debt_close_factor_pct: LIQUIDATION_CLOSE_FACTOR,
            insolvency_risk_unhealthy_ltv_pct: CLOSE_TO_INSOLVENCY_RISKY_LTV,
            max_liquidatable_debt_market_value_at_once: MAX_LIQUIDATABLE_VALUE_AT_ONCE,
            global_allowed_borrow_value: GLOBAL_ALLOWED_BORROW_VALUE,
            global_unhealthy_borrow_value: GLOBAL_UNHEALTHY_BORROW_VALUE,
            min_full_liquidation_value_threshold: LIQUIDATION_CLOSE_VALUE,
            padding1: [0; 2],
        }
    }
}

impl LendingMarket {
    pub fn init(&mut self, params: InitLendingMarketParams) {
        *self = Self::default();
        self.version = PROGRAM_VERSION as u64;
        self.bump_seed = params.bump_seed as u64;
        self.owner = params.owner;
        self.quote_currency = params.quote_currency;
    }

    pub fn is_borrowing_disabled(&self) -> bool {
        self.borrow_disabled != false as u8
    }
}

pub struct InitLendingMarketParams {
    pub bump_seed: u8,
    pub owner: Pubkey,
    pub quote_currency: [u8; 32],
}
