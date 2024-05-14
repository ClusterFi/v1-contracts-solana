use anchor_lang::{prelude::*, Accounts};

use crate::{
    state::LendingMarket, validation::validate_numerical_bool, LendingError,
    UpdateLendingMarketMode, VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE,
};

pub fn process(
    ctx: Context<UpdateLendingMarket>,
    mode: u64,
    value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
) -> Result<()> {
    let mode = UpdateLendingMarketMode::try_from(mode)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let market = &mut ctx.accounts.lending_market.load_mut()?;

    msg!(
        "Updating lending market with mode {:?} and value {:?}",
        mode,
        &value[0..32]
    );

    match mode {
        UpdateLendingMarketMode::UpdateOwner => {
            let value: [u8; 32] = value[0..32].try_into().unwrap();
            let value = Pubkey::from(value);
            market.owner_cached = value;
            msg!("Value is {:?}", value);
        }
        UpdateLendingMarketMode::UpdateEmergencyMode => {
            let emergency_mode = value[0];
            msg!("Value is {:?}", emergency_mode);
            if emergency_mode == 0 {
                market.emergency_mode = 0
            } else if emergency_mode == 1 {
                market.emergency_mode = 1;
            } else {
                return err!(LendingError::InvalidFlag);
            }
        }
        UpdateLendingMarketMode::UpdateLiquidationCloseFactor => {
            let liquidation_close_factor = value[0];
            msg!("Value is {:?}", liquidation_close_factor);
            if !(5..=100).contains(&liquidation_close_factor) {
                return err!(LendingError::InvalidFlag);
            }
            market.liquidation_max_debt_close_factor_pct = liquidation_close_factor;
        }
        UpdateLendingMarketMode::UpdateLiquidationMaxValue => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            if value == 0 {
                return err!(LendingError::InvalidFlag);
            }
            market.max_liquidatable_debt_market_value_at_once = value;
        }
        UpdateLendingMarketMode::UpdateGlobalAllowedBorrow => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.global_allowed_borrow_value = value;
        }
        UpdateLendingMarketMode::UpdateGlobalUnhealthyBorrow => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.global_unhealthy_borrow_value = value;
        }
        UpdateLendingMarketMode::UpdateMinFullLiquidationThreshold => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            if value == 0 {
                return err!(LendingError::InvalidFlag);
            }
            market.min_full_liquidation_value_threshold = value;
        }
        UpdateLendingMarketMode::UpdatePriceRefreshTriggerToMaxAgePct => {
            let value = value[0];
            msg!("Value is {:?}", value);
            if value > 100 {
                msg!("Price refresh trigger to max age pct must be in range [0, 100]");
                return err!(LendingError::InvalidConfig);
            }
            market.price_refresh_trigger_to_max_age_pct = value;
        }
        UpdateLendingMarketMode::UpdateAutodeleverageEnabled => {
            let autodeleverage_enabled = value[0];
            msg!("Prev Value is {:?}", market.autodeleverage_enabled);
            msg!("New Value is {:?}", autodeleverage_enabled);
            if autodeleverage_enabled == 0 {
                market.autodeleverage_enabled = 0
            } else if autodeleverage_enabled == 1 {
                market.autodeleverage_enabled = 1;
            } else {
                msg!(
                    "Autodeleverage enabled flag must be 0 or 1, got {:?}",
                    autodeleverage_enabled
                );
                return err!(LendingError::InvalidFlag);
            }
        }
        UpdateLendingMarketMode::UpdateBorrowingDisabled => {
            let borrow_disabled = value[0];
            msg!("Prev Value is {:?}", market.borrow_disabled);
            msg!("New Value is {:?}", borrow_disabled);
            validate_numerical_bool(borrow_disabled)?;
            market.borrow_disabled = borrow_disabled;
        }
        _ => {
            msg!("Invalid mode, got {:?}", mode);
            return err!(LendingError::InvalidFlag);
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarket<'info> {
    owner: Signer<'info>,

    #[account(mut, has_one = owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
