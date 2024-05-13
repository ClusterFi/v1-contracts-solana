use anchor_lang::prelude::*;

use crate::consts::PROGRAM_VERSION;

#[account(zero_copy)]
pub struct LendingMarket {
    pub version: u64,
    pub bump_seed: u64,

    pub lending_market_owner: Pubkey,

    pub lending_market_owner_cached: Pubkey,

    pub quote_currency: [u8; 32],

    pub emergency_mode: u8,

    pub autodeleverage_enabled: u8,

    pub borrow_disabled: u8,

    pub padding1: [u8; 200],
}

impl Default for LendingMarket {
    fn default() -> Self {
        Self {
            version: 0,
            bump_seed: 0,
            lending_market_owner: Pubkey::default(),
            lending_market_owner_cached: Pubkey::default(),
            quote_currency: [0; 32],
            emergency_mode: 0,
            autodeleverage_enabled: 0,
            borrow_disabled: 0,
            padding1: [0; 240],
        }
    }
}

impl LendingMarket {
    pub fn init(&mut self, params: InitLendingMarketParams) {
        *self = Self::default();
        self.version = PROGRAM_VERSION as u64;
        self.bump_seed = params.bump_seed as u64;
        self.lending_market_owner = params.lending_market_owner;
        self.quote_currency = params.quote_currency;
    }

    pub fn is_borrowing_disabled(&self) -> bool {
        self.borrow_disabled != false as u8
    }
}

pub struct InitLendingMarketParams {
    pub bump_seed: u8,
    pub lending_market_owner: Pubkey,
    pub quote_currency: [u8; 32],
}
