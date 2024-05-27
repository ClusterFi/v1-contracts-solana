use anchor_lang::prelude::*;

mod market;

pub use market::*;
use num_enum::TryFromPrimitive;
use strum::EnumString;

use crate::consts::RESERVE_CONFIG_SIZE;

pub const VALUE_BYTE_ARRAY_LEN_RESERVE: usize = RESERVE_CONFIG_SIZE;
pub const VALUE_BYTE_ARRAY_LEN_SHORT_UPDATE: usize = 32;

pub const VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE: usize = 72;

#[derive(
    TryFromPrimitive,
    AnchorSerialize,
    AnchorDeserialize,
    EnumString,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Debug,
)]
#[repr(u64)]
pub enum UpdateLendingMarketMode {
    UpdateOwner = 0,
    UpdateEmergencyMode = 1,
    UpdateLiquidationCloseFactor = 2,
    UpdateLiquidationMaxValue = 3,
    UpdateGlobalUnhealthyBorrow = 4,
    UpdateGlobalAllowedBorrow = 5,
    UpdateRiskCouncil = 6,
    UpdateMinFullLiquidationThreshold = 7,
    UpdateInsolvencyRiskLtv = 8,
    UpdateElevationGroup = 9,
    UpdateReferralFeeBps = 10,
    UpdateMultiplierPoints = 11,
    UpdatePriceRefreshTriggerToMaxAgePct = 12,
    UpdateAutodeleverageEnabled = 13,
    UpdateBorrowingDisabled = 14,
    UpdateMinNetValueObligationPostAction = 15,
}
