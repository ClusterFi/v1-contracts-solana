use crate::constants::PROGRAM_VERSION;
use crate::state::{
    CalculateBorrowResult, LendingAction, Obligation, RedeemReserveCollateralAccounts,
    ReserveStatus,
};
use crate::utils::fraction::Fraction;
use crate::utils::BigFraction;
use crate::{
    errors::LendingError,
    state::{LendingMarket, PriceStatusFlags, Reserve},
    utils::GetPriceResult,
};
use crate::{
    BorrowObligationLiquidityCtx, DepositObligationCollateralAccounts,
    DepositReserveLiquidityAccounts, LiquidateObligationCtx, RepayObligationLiquidityCtx,
    WithdrawObligationCollateralAccounts,
};
use anchor_lang::solana_program::clock::Slot;
use anchor_lang::{prelude::*, solana_program::clock::UnixTimestamp};

/*

pub fn flash_borrow_reserve_liquidity_checks(
    ctx: &Context<FlashBorrowReserveLiquidity>,
) -> Result<()> {
    let reserve = ctx.accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == ctx.accounts.user_destination_liquidity.key() {
        msg!(
            "Borrow reserve liquidity supply cannot be used as the destination liquidity provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is obsolete");
        return err!(LendingError::ReserveObsolete);
    }

    if reserve.config.fees.flash_loan_fee_sf == u64::MAX {
        msg!("Flash loans are disabled for this reserve");
        return err!(LendingError::FlashLoansDisabled);
    }

    Ok(())
}

pub fn flash_repay_reserve_liquidity_checks(
    ctx: &Context<FlashRepayReserveLiquidity>,
) -> Result<()> {
    let reserve = ctx.accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}

*/

pub fn post_transfer_vault_balance_liquidity_reserve_checks(
    final_reserve_vault_balance: u64,
    final_reserve_available_liquidity: u64,
    initial_reserve_vault_balance: u64,
    initial_reserve_available_liquidity: u64,
    action_type: LendingAction,
) -> anchor_lang::Result<()> {
    let pre_transfer_reserve_diff =
        initial_reserve_vault_balance - initial_reserve_available_liquidity;
    let post_transfer_reserve_diff =
        final_reserve_vault_balance - final_reserve_available_liquidity;

    require_eq!(
        pre_transfer_reserve_diff,
        post_transfer_reserve_diff,
        LendingError::ReserveTokenBalanceMismatch
    );

    match action_type {
        LendingAction::Additive(amount_transferred) => {
            let expected_reserve_vault_balance = initial_reserve_vault_balance + amount_transferred;
            require_eq!(
                expected_reserve_vault_balance,
                final_reserve_vault_balance,
                LendingError::ReserveVaultBalanceMismatch,
            );

            let expected_reserve_available_liquidity =
                initial_reserve_available_liquidity + amount_transferred;
            require_eq!(
                expected_reserve_available_liquidity,
                final_reserve_available_liquidity,
                LendingError::ReserveAccountingMismatch
            );
        }
        LendingAction::Subtractive(amount_transferred) => {
            let expected_reserve_vault_balance = initial_reserve_vault_balance - amount_transferred;
            require_eq!(
                expected_reserve_vault_balance,
                final_reserve_vault_balance,
                LendingError::ReserveVaultBalanceMismatch
            );

            let expected_reserve_available_liquidity =
                initial_reserve_available_liquidity - amount_transferred;
            require_eq!(
                expected_reserve_available_liquidity,
                final_reserve_available_liquidity,
                LendingError::ReserveAccountingMismatch
            );
        }
    }

    Ok(())
}

pub fn deposit_reserve_liquidity_checks(accounts: &DepositReserveLiquidityAccounts) -> Result<()> {
    let reserve = accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == accounts.user_source_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if reserve.collateral.supply_vault == accounts.user_destination_collateral.key() {
        msg!("Reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}
pub fn redeem_reserve_collateral_checks(accounts: &RedeemReserveCollateralAccounts) -> Result<()> {
    let reserve = &accounts.reserve.load()?;

    if reserve.collateral.supply_vault == accounts.user_source_collateral.key() {
        msg!("Reserve collateral supply cannot be used as the source collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if reserve.liquidity.supply_vault == accounts.user_destination_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the destination liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn borrow_obligation_liquidity_checks(
    ctx: &Context<BorrowObligationLiquidityCtx>,
) -> Result<()> {
    let borrow_reserve = &ctx.accounts.borrow_reserve.load()?;

    if borrow_reserve.liquidity.supply_vault == ctx.accounts.user_destination_liquidity.key() {
        msg!(
            "Borrow reserve liquidity supply cannot be used as the destination liquidity provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if borrow_reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if borrow_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn deposit_obligation_collateral_checks(
    accounts: &DepositObligationCollateralAccounts,
) -> Result<()> {
    let deposit_reserve = &accounts.deposit_reserve.load()?;

    if deposit_reserve.collateral.supply_vault == accounts.user_source_collateral.key() {
        msg!("Deposit reserve collateral supply cannot be used as the source collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if deposit_reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if deposit_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn withdraw_obligation_collateral_checks(
    accounts: &WithdrawObligationCollateralAccounts,
) -> Result<()> {
    let withdraw_reserve = accounts.withdraw_reserve.load()?;

    if withdraw_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if withdraw_reserve.collateral.supply_vault == accounts.user_destination_collateral.key() {
        msg!("Withdraw reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}

pub fn repay_obligation_liquidity_checks(ctx: &Context<RepayObligationLiquidityCtx>) -> Result<()> {
    let repay_reserve = ctx.accounts.repay_reserve.load()?;

    if repay_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Repay reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if repay_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn liquidate_obligation_checks(ctx: &Context<LiquidateObligationCtx>) -> Result<()> {
    let repay_reserve = ctx.accounts.repay_reserve.load()?;
    let withdraw_reserve = ctx.accounts.withdraw_reserve.load()?;

    if repay_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Repay reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if repay_reserve.collateral.supply_vault == ctx.accounts.user_destination_collateral.key() {
        msg!(
            "Repay reserve collateral supply cannot be used as the destination collateral provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if repay_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Withdraw reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if withdraw_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Withdraw reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if withdraw_reserve.collateral.supply_vault == ctx.accounts.user_destination_collateral.key() {
        msg!("Withdraw reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if withdraw_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Withdraw reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn initial_liquidation_reserve_liquidity_available_amount(
    repay_reserve: &AccountLoader<Reserve>,
    withdraw_reserve: &AccountLoader<Reserve>,
) -> (u64, u64) {
    let repay_reserve = repay_reserve.load().unwrap();
    let withdraw_reserve = withdraw_reserve.load().unwrap();
    let repay_reserve_liquidity = repay_reserve.liquidity.available_amount;
    let withdraw_reserve_liquidity = withdraw_reserve.liquidity.available_amount;

    (repay_reserve_liquidity, withdraw_reserve_liquidity)
}
