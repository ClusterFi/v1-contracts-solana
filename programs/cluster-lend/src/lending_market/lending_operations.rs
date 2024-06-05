use std::{
    cell::RefMut,
    cmp::min,
    ops::{Add, Div, Mul},
};

use crate::{
    errors::LendingError,
    lending_market::liquidation_operations,
    state::{LendingMarket, PriceStatusFlags, Reserve},
    utils::GetPriceResult,
    CalculateLiquidationResult, LiquidateAndRedeemResult,
};
use crate::{
    state::{
        CalculateBorrowResult, Obligation, RefreshObligationBorrowsResult,
        RefreshObligationDepositsResult, ReserveStatus,
    },
    xmsg,
};
use crate::{
    utils::{fraction::Fraction, AnyAccountLoader},
    CalculateRepayResult,
};
use crate::{
    utils::{BigFraction, FractionExtra},
    LiquidateObligationResult,
};
use anchor_lang::solana_program::clock::Slot;
use anchor_lang::{prelude::*, solana_program::clock::UnixTimestamp};
use utils::{
    calculate_obligation_collateral_market_value, calculate_obligation_liquidity_market_value,
    check_obligation_collateral_deposit_reserve, check_obligation_fully_refreshed_and_not_null,
    check_obligation_liquidity_borrow_reserve, post_borrow_obligation_invariants,
    post_deposit_obligation_invariants, post_repay_obligation_invariants,
    post_withdraw_obligation_invariants,
};

use super::withdrawal_operations::utils::{add_to_withdrawal_accum, sub_from_withdrawal_accum};

pub fn refresh_reserve(
    reserve: &mut Reserve,
    clock: &Clock,
    price: Option<GetPriceResult>,
) -> Result<()> {
    let slot = clock.slot;

    reserve.accrue_interest(slot)?;

    let price_status = if let Some(GetPriceResult {
        price,
        status,
        timestamp,
    }) = price
    {
        reserve.liquidity.market_price_sf = price.to_bits();
        reserve.liquidity.market_price_last_updated_ts = timestamp;

        Some(status)
    } else if !is_saved_price_age_valid(reserve, clock.unix_timestamp) {
        Some(PriceStatusFlags::empty())
    } else {
        None
    };

    reserve.last_update.update_slot(slot, price_status);

    Ok(())
}

pub fn refresh_reserve_limit_timestamps(reserve: &mut Reserve, slot: Slot) -> Result<()> {
    reserve.update_deposit_limit_crossed_slot(slot)?;
    reserve.update_borrow_limit_crossed_slot(slot)?;
    Ok(())
}

pub fn deposit_reserve_liquidity(
    reserve: &mut Reserve,
    clock: &Clock,
    liquidity_amount: u64,
) -> Result<u64> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let deposit_limit_f = Fraction::from(reserve.config.deposit_limit);
    let reserve_liquidity_supply_f = reserve.liquidity.total_supply()?;

    let new_reserve_liquidity_supply_f = liquidity_amount_f + reserve_liquidity_supply_f;

    if new_reserve_liquidity_supply_f > deposit_limit_f {
        msg!(
            "Cannot deposit liquidity above the reserve deposit limit. New total deposit: {} > limit: {}",
            new_reserve_liquidity_supply_f,
            reserve.config.deposit_limit
        );
        return err!(LendingError::DepositLimitExceeded);
    }

    sub_from_withdrawal_accum(
        &mut reserve.config.deposit_withdrawal_cap,
        liquidity_amount,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    let collateral_amount = reserve.deposit_liquidity(liquidity_amount)?;

    reserve.last_update.mark_stale();

    Ok(collateral_amount)
}

pub fn redeem_reserve_collateral(
    reserve: &mut Reserve,
    collateral_amount: u64,
    clock: &Clock,
    add_amount_to_withdrawal_caps: bool,
) -> Result<u64> {
    if collateral_amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let liquidity_amount = reserve.redeem_collateral(collateral_amount)?;
    refresh_reserve_limit_timestamps(reserve, clock.slot)?;
    reserve.last_update.mark_stale();

    if add_amount_to_withdrawal_caps {
        add_to_withdrawal_accum(
            &mut reserve.config.deposit_withdrawal_cap,
            liquidity_amount,
            u64::try_from(clock.unix_timestamp).unwrap(),
        )?;
    }

    Ok(liquidity_amount)
}

pub fn refresh_obligation_deposits<'info, T>(
    obligation: &mut Obligation,
    slot: Slot,
    mut reserves_iter: impl Iterator<Item = T>,
) -> Result<RefreshObligationDepositsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let mut lowest_deposit_ltv_accumulator = u8::MAX;
    let mut deposited_value = Fraction::ZERO;
    let mut allowed_borrow_value = Fraction::ZERO;
    let mut unhealthy_borrow_value = Fraction::ZERO;
    let mut num_of_obsolete_reserves = 0;
    let mut prices_state = PriceStatusFlags::all();

    for (index, deposit) in obligation
        .deposits
        .iter_mut()
        .enumerate()
        .filter(|(_, deposit)| deposit.deposit_reserve != Pubkey::default())
    {
        let deposit_reserve = reserves_iter
            .next()
            .ok_or(LendingError::InvalidAccountInput)?;

        let deposit_reserve_info_key = deposit_reserve.get_pubkey();

        let deposit_reserve = deposit_reserve
            .get()
            .map_err(|_| LendingError::InvalidAccountInput)?;

        if deposit_reserve.config.status() == ReserveStatus::Obsolete {
            num_of_obsolete_reserves += 1;
        }

        check_obligation_collateral_deposit_reserve(
            deposit,
            &deposit_reserve,
            deposit_reserve_info_key,
            index,
            slot,
        )?;

        let market_value_f =
            calculate_obligation_collateral_market_value(&deposit_reserve, deposit)?;
        deposit.market_value_sf = market_value_f.to_bits();

        let (coll_ltv_pct, coll_liquidation_threshold_pct) = (
            deposit_reserve.config.loan_to_value_pct,
            deposit_reserve.config.liquidation_threshold_pct,
        );

        lowest_deposit_ltv_accumulator = min(
            lowest_deposit_ltv_accumulator.min(deposit_reserve.config.loan_to_value_pct),
            coll_ltv_pct,
        );

        deposited_value = deposited_value.add(market_value_f);
        allowed_borrow_value += market_value_f * Fraction::from_percent(coll_ltv_pct);
        unhealthy_borrow_value +=
            market_value_f * Fraction::from_percent(coll_liquidation_threshold_pct);

        obligation.deposits_asset_tiers[index] = deposit_reserve.config.asset_tier;

        prices_state &= deposit_reserve.last_update.get_price_status();

        xmsg!(
            "Deposit: {} amount: {} value: {}",
            &deposit_reserve.config.token_info.symbol(),
            deposit_reserve
                .collateral_exchange_rate()?
                .fraction_collateral_to_liquidity(deposit.deposited_amount.into())
                .to_display(),
            market_value_f.to_display()
        );
    }

    Ok(RefreshObligationDepositsResult {
        lowest_deposit_ltv_accumulator,
        num_of_obsolete_reserves,
        deposited_value_f: deposited_value,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
        prices_state,
    })
}

pub fn refresh_obligation_borrows<'info, T>(
    obligation: &mut Obligation,
    slot: u64,
    mut reserves_iter: impl Iterator<Item = T>,
) -> Result<RefreshObligationBorrowsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let mut borrowed_assets_market_value = Fraction::ZERO;
    let mut borrow_factor_adjusted_debt_value = Fraction::ZERO;
    let mut prices_state = PriceStatusFlags::all();

    for (index, borrow) in obligation
        .borrows
        .iter_mut()
        .enumerate()
        .filter(|(_, borrow)| borrow.borrow_reserve != Pubkey::default())
    {
        let borrow_reserve = reserves_iter
            .next()
            .ok_or(LendingError::InvalidAccountInput)?;

        let borrow_reserve_info_key = borrow_reserve.get_pubkey();

        let borrow_reserve = &mut borrow_reserve
            .get_mut()
            .map_err(|_| LendingError::InvalidAccountInput)?;

        check_obligation_liquidity_borrow_reserve(
            borrow,
            borrow_reserve,
            borrow_reserve_info_key,
            index,
            slot,
        )?;

        let cumulative_borrow_rate_bf =
            BigFraction::from(borrow_reserve.liquidity.cumulative_borrow_rate_bsf);

        borrow.accrue_interest(cumulative_borrow_rate_bf)?;

        let market_value_f = calculate_obligation_liquidity_market_value(borrow_reserve, borrow)?;

        borrow.market_value_sf = market_value_f.to_bits();

        borrowed_assets_market_value += market_value_f;

        let borrow_factor_adjusted_market_value: Fraction =
            market_value_f * borrow_reserve.config.get_borrow_factor();

        borrow.borrow_factor_adjusted_market_value_sf =
            borrow_factor_adjusted_market_value.to_bits();

        borrow_factor_adjusted_debt_value += borrow_factor_adjusted_market_value;

        obligation.borrows_asset_tiers[index] = borrow_reserve.config.asset_tier;

        obligation.has_debt = 1;

        prices_state &= borrow_reserve.last_update.get_price_status();

        xmsg!(
            "Borrow: {} amount: {} value: {} value_bf: {}",
            &borrow_reserve.config.token_info.symbol(),
            Fraction::from_bits(borrow.borrowed_amount_sf),
            market_value_f.to_display(),
            borrow_factor_adjusted_market_value.to_display()
        );
    }

    Ok(RefreshObligationBorrowsResult {
        borrowed_assets_market_value_f: borrowed_assets_market_value,
        borrow_factor_adjusted_debt_value_f: borrow_factor_adjusted_debt_value,
        prices_state,
    })
}

pub fn refresh_obligation<'info, T>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    mut reserves_iter: impl Iterator<Item = T>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let RefreshObligationDepositsResult {
        lowest_deposit_ltv_accumulator,
        num_of_obsolete_reserves,
        deposited_value_f,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
        prices_state: deposits_prices_state,
    } = refresh_obligation_deposits(obligation, slot, &mut reserves_iter)?;

    let RefreshObligationBorrowsResult {
        borrow_factor_adjusted_debt_value_f,
        borrowed_assets_market_value_f,
        prices_state: borrows_prices_state,
    } = refresh_obligation_borrows(obligation, slot, &mut reserves_iter)?;

    obligation.borrowed_assets_market_value_sf = borrowed_assets_market_value_f.to_bits();

    obligation.deposited_value_sf = deposited_value_f.to_bits();

    obligation.borrow_factor_adjusted_debt_value_sf = borrow_factor_adjusted_debt_value_f.to_bits();

    obligation.allowed_borrow_value_sf = min(
        allowed_borrow_value,
        Fraction::from(lending_market.global_allowed_borrow_value),
    )
    .to_bits();

    obligation.unhealthy_borrow_value_sf = min(
        unhealthy_borrow_value,
        Fraction::from(lending_market.global_unhealthy_borrow_value),
    )
    .to_bits();

    obligation.lowest_reserve_deposit_ltv = lowest_deposit_ltv_accumulator.into();
    obligation.num_of_obsolete_reserves = num_of_obsolete_reserves;

    let prices_state = deposits_prices_state.intersection(borrows_prices_state);
    obligation.last_update.update_slot(slot, Some(prices_state));

    Ok(())
}

pub fn borrow_obligation_liquidity(
    lending_market: &LendingMarket,
    borrow_reserve: &mut Reserve,
    obligation: &mut Obligation,
    liquidity_amount: u64,
    clock: &Clock,
    borrow_reserve_pk: Pubkey,
) -> Result<CalculateBorrowResult> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if borrow_reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::ALL_CHECKS)?
    {
        msg!(
            "Borrow reserve is stale and must be refreshed in the current slot, price_status: {:08b}",
            borrow_reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    if lending_market.is_borrowing_disabled() {
        msg!("Borrowing is disabled");
        return err!(LendingError::BorrowingDisabled);
    }

    let reserve_liquidity_borrowed_f = borrow_reserve.liquidity.total_borrow();
    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let borrow_limit_f = Fraction::from(borrow_reserve.config.borrow_limit);

    let new_borrowed_amount_f = liquidity_amount_f + reserve_liquidity_borrowed_f;
    if liquidity_amount != u64::MAX && new_borrowed_amount_f > borrow_limit_f {
        msg!(
            "Cannot borrow above the borrow limit. New total borrow: {} > limit: {}",
            new_borrowed_amount_f.to_display(),
            borrow_reserve.config.borrow_limit
        );
        return err!(LendingError::BorrowLimitExceeded);
    }
    check_obligation_fully_refreshed_and_not_null(obligation, clock.slot)?;

    let remaining_borrow_value = obligation.remaining_borrow_value();
    if remaining_borrow_value == Fraction::ZERO {
        msg!("Remaining borrow value is zero");
        return err!(LendingError::BorrowTooLarge);
    }

    let remaining_reserve_capacity = borrow_limit_f.saturating_sub(reserve_liquidity_borrowed_f);

    if remaining_reserve_capacity == Fraction::ZERO {
        msg!("Borrow reserve is at full capacity");
        return err!(LendingError::BorrowLimitExceeded);
    }

    let CalculateBorrowResult {
        borrow_amount_f,
        receive_amount,
        borrow_fee,
    } = borrow_reserve.calculate_borrow(
        liquidity_amount,
        remaining_borrow_value,
        remaining_reserve_capacity,
    )?;

    add_to_withdrawal_accum(
        &mut borrow_reserve.config.debt_withdrawal_cap,
        borrow_amount_f.to_floor(),
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    if receive_amount == 0 {
        msg!("Borrow amount is too small to receive liquidity after fees");
        return err!(LendingError::BorrowTooSmall);
    }

    borrow_reserve.liquidity.borrow(borrow_amount_f)?;
    borrow_reserve.last_update.mark_stale();

    let cumulative_borrow_rate_bf =
        BigFraction::from(borrow_reserve.liquidity.cumulative_borrow_rate_bsf);

    let (obligation_liquidity, liquidity_index) = obligation.find_or_add_liquidity_to_borrows(
        borrow_reserve_pk,
        cumulative_borrow_rate_bf,
        borrow_reserve.config.get_asset_tier(),
    )?;

    obligation_liquidity.borrow(borrow_amount_f);
    obligation.has_debt = 1;
    obligation.last_update.mark_stale();

    post_borrow_obligation_invariants(
        borrow_amount_f,
        obligation,
        borrow_reserve,
        Fraction::from_bits(obligation.borrows[liquidity_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(CalculateBorrowResult {
        borrow_amount_f,
        receive_amount,
        borrow_fee,
    })
}

pub fn deposit_obligation_collateral(
    deposit_reserve: &mut Reserve,
    obligation: &mut Obligation,
    slot: Slot,
    collateral_amount: u64,
    deposit_reserve_pk: Pubkey,
    lending_market: &LendingMarket,
) -> Result<()> {
    if collateral_amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if deposit_reserve
        .last_update
        .is_stale(slot, PriceStatusFlags::NONE)?
    {
        msg!("Deposit reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let (collateral, collateral_index) = obligation.find_or_add_collateral_to_deposits(
        deposit_reserve_pk,
        deposit_reserve.config.get_asset_tier(),
    )?;

    collateral.deposit(collateral_amount)?;
    obligation.last_update.mark_stale();

    deposit_reserve.last_update.mark_stale();

    post_deposit_obligation_invariants(
        deposit_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(Fraction::from(collateral_amount)),
        obligation,
        deposit_reserve,
        Fraction::from_bits(obligation.deposits[collateral_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(())
}

pub fn withdraw_obligation_collateral(
    lending_market: &LendingMarket,
    withdraw_reserve: &Reserve,
    obligation: &mut Obligation,
    collateral_amount: u64,
    slot: Slot,
    withdraw_reserve_pk: Pubkey,
) -> Result<u64> {
    if collateral_amount == 0 {
        return err!(LendingError::InvalidAmount);
    }

    let is_borrows_empty = obligation.borrows_empty();

    let required_price_status = if is_borrows_empty {
        PriceStatusFlags::NONE
    } else {
        PriceStatusFlags::ALL_CHECKS
    };

    if withdraw_reserve
        .last_update
        .is_stale(slot, required_price_status)?
    {
        msg!(
            "Withdraw reserve is stale and must be refreshed in the current slot, price status: {:08b}",
            withdraw_reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    if obligation
        .last_update
        .is_stale(slot, required_price_status)?
    {
        msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
        return err!(LendingError::ObligationStale);
    }

    let (collateral, collateral_index) =
        obligation.find_collateral_in_deposits(withdraw_reserve_pk)?;
    if collateral.deposited_amount == 0 {
        return err!(LendingError::ObligationCollateralEmpty);
    }

    if obligation.num_of_obsolete_reserves > 0
        && withdraw_reserve.config.status() == ReserveStatus::Active
    {
        return err!(LendingError::ObligationInDeprecatedReserve);
    }

    let withdraw_amount = if is_borrows_empty {
        if collateral_amount == u64::MAX {
            collateral.deposited_amount
        } else {
            collateral.deposited_amount.min(collateral_amount)
        }
    } else if obligation.deposited_value_sf == 0 {
        msg!("Obligation deposited value is zero");
        return err!(LendingError::ObligationDepositsZero);
    } else {
        let reserve_loan_to_value_pct = withdraw_reserve.config.loan_to_value_pct;

        let max_withdraw_value = obligation.max_withdraw_value(reserve_loan_to_value_pct)?;

        if max_withdraw_value == Fraction::ZERO {
            msg!("Maximum withdraw value is zero");
            return err!(LendingError::WithdrawTooLarge);
        }

        let collateral_value = Fraction::from_bits(collateral.market_value_sf);
        let withdraw_amount = if collateral_amount == u64::MAX {
            let withdraw_value = max_withdraw_value.min(collateral_value);
            let withdraw_ratio = withdraw_value / collateral_value;

            let ratioed_amount_f = withdraw_ratio * u128::from(collateral.deposited_amount);
            let ratioed_amount: u64 = ratioed_amount_f.to_floor();

            min(collateral.deposited_amount, ratioed_amount)
        } else {
            let withdraw_amount = collateral_amount.min(collateral.deposited_amount);
            let withdraw_ratio =
                Fraction::from(withdraw_amount) / u128::from(collateral.deposited_amount);
            let withdraw_value = collateral_value * withdraw_ratio;
            if withdraw_value > max_withdraw_value {
                msg!("Withdraw value cannot exceed maximum withdraw value, collateral_amount={}, collateral.deposited_amount={} withdraw_pct={}, collateral_value={}, max_withdraw_value={} withdraw_value={}",
                    collateral_amount,
                    collateral.deposited_amount,
                    withdraw_ratio,
                    collateral_value,
                    max_withdraw_value,
                    withdraw_value);
                return err!(LendingError::WithdrawTooLarge);
            }
            withdraw_amount
        };

        if withdraw_amount == 0 {
            msg!("Withdraw amount is too small to transfer collateral");
            return err!(LendingError::WithdrawTooSmall);
        }
        withdraw_amount
    };

    obligation.withdraw(withdraw_amount, collateral_index)?;
    obligation.last_update.mark_stale();

    post_withdraw_obligation_invariants(
        withdraw_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(Fraction::from(withdraw_amount)),
        obligation,
        withdraw_reserve,
        Fraction::from_bits(obligation.deposits[collateral_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(withdraw_amount)
}

pub fn repay_obligation_liquidity(
    repay_reserve: &mut Reserve,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    repay_reserve_pk: Pubkey,
    lending_market: &LendingMarket,
) -> Result<u64> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if repay_reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Repay reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let (liquidity, liquidity_index) =
        obligation.find_liquidity_in_borrows_mut(repay_reserve_pk)?;
    if liquidity.borrowed_amount_sf == 0 {
        msg!("Liquidity borrowed amount is zero");
        return err!(LendingError::ObligationLiquidityEmpty);
    }

    let cumulative_borrow_rate =
        BigFraction::from(repay_reserve.liquidity.cumulative_borrow_rate_bsf);
    liquidity.accrue_interest(cumulative_borrow_rate)?;

    let CalculateRepayResult {
        settle_amount_f: settle_amount,
        repay_amount,
    } = repay_reserve.calculate_repay(
        liquidity_amount,
        Fraction::from_bits(liquidity.borrowed_amount_sf),
    )?;

    if repay_amount == 0 {
        msg!("Repay amount is too small to transfer liquidity");
        return err!(LendingError::RepayTooSmall);
    }

    sub_from_withdrawal_accum(
        &mut repay_reserve.config.debt_withdrawal_cap,
        repay_amount,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    repay_reserve.liquidity.repay(repay_amount, settle_amount)?;
    repay_reserve.last_update.mark_stale();

    obligation.repay(settle_amount, liquidity_index)?;
    obligation.update_has_debt();
    obligation.last_update.mark_stale();

    post_repay_obligation_invariants(
        settle_amount,
        obligation,
        repay_reserve,
        Fraction::from_bits(obligation.borrows[liquidity_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(repay_amount)
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_and_redeem(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    min_acceptable_received_collateral_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidateAndRedeemResult> {
    let LiquidateObligationResult {
        repay_amount,
        withdraw_collateral_amount,
        withdraw_amount,
        liquidation_bonus_rate,
        ..
    } = liquidate_obligation(
        lending_market,
        repay_reserve,
        withdraw_reserve,
        obligation,
        clock,
        liquidity_amount,
        min_acceptable_received_collateral_amount,
        max_allowed_ltv_override_pct_opt,
    )?;

    let withdraw_reserve = &mut withdraw_reserve.get_mut()?;

    let total_withdraw_liquidity_amount = post_liquidate_redeem(
        withdraw_reserve,
        repay_amount,
        withdraw_collateral_amount,
        liquidation_bonus_rate,
        clock,
    )?;

    Ok(LiquidateAndRedeemResult {
        repay_amount,
        withdraw_amount,
        total_withdraw_liquidity_amount,
        withdraw_collateral_amount,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_obligation(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    min_acceptable_received_collateral_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidateObligationResult> {
    xmsg!(
        "Liquidating liquidation_close_factor_pct: {}, liquidation_max_value: {}",
        lending_market.liquidation_max_debt_close_factor_pct,
        lending_market.max_liquidatable_debt_market_value_at_once
    );
    let repay_reserve_ref = repay_reserve.get()?;
    let withdraw_reserve_ref = withdraw_reserve.get()?;

    let slot = clock.slot;

    if withdraw_reserve_ref.config.loan_to_value_pct == 0
        || withdraw_reserve_ref.config.liquidation_threshold_pct == 0
    {
        xmsg!("Max LTV of the withdraw reserve is 0 and can't be used for liquidation");
        return err!(LendingError::CollateralNonLiquidatable);
    }

    utils::assert_obligation_liquidatable(
        &repay_reserve_ref,
        &withdraw_reserve_ref,
        obligation,
        liquidity_amount,
        slot,
    )?;

    let (liquidity, liquidity_index) =
        obligation.find_liquidity_in_borrows(repay_reserve.get_pubkey())?;
    if liquidity.borrow_factor_adjusted_market_value_sf == 0 {
        msg!("Obligation borrow value is zero");
        return err!(LendingError::ObligationLiquidityEmpty);
    }

    let (collateral, collateral_index) =
        obligation.find_collateral_in_deposits(withdraw_reserve.get_pubkey())?;
    if collateral.market_value_sf == 0 {
        msg!("Obligation deposit value is zero");
        return err!(LendingError::ObligationCollateralEmpty);
    }

    let CalculateLiquidationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        liquidation_bonus_rate,
    } = liquidation_operations::calculate_liquidation(
        &withdraw_reserve_ref,
        &repay_reserve_ref,
        liquidity_amount,
        lending_market,
        obligation,
        liquidity,
        collateral,
        slot,
        max_allowed_ltv_override_pct_opt,
    )?;

    drop(repay_reserve_ref);
    drop(withdraw_reserve_ref);

    {
        let mut repay_reserve_ref_mut = repay_reserve.get_mut()?;

        utils::repay_and_withdraw_from_obligation_post_liquidation(
            obligation,
            &mut repay_reserve_ref_mut,
            settle_amount,
            withdraw_amount,
            repay_amount,
            liquidity_index,
            collateral_index,
        )?;
    }

    let withdraw_collateral_amount = {
        let mut withdraw_reserve_ref_mut = withdraw_reserve.get_mut()?;
        refresh_reserve(&mut withdraw_reserve_ref_mut, clock, None)?;
        let collateral_exchange_rate = withdraw_reserve_ref_mut.collateral_exchange_rate()?;
        let max_redeemable_collateral = collateral_exchange_rate
            .liquidity_to_collateral(withdraw_reserve_ref_mut.liquidity.available_amount);
        min(withdraw_amount, max_redeemable_collateral)
    };

    if withdraw_collateral_amount < min_acceptable_received_collateral_amount {
        msg!("Withdraw amount below minimum acceptable collateral amount");
        return err!(LendingError::LiquidationSlippageError);
    }

    Ok(LiquidateObligationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        withdraw_collateral_amount,
        liquidation_bonus_rate,
    })
}

pub(crate) fn post_liquidate_redeem(
    withdraw_reserve: &mut Reserve,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
    liquidation_bonus_rate: Fraction,
    clock: &Clock,
) -> Result<Option<(u64, u64)>> {
    if withdraw_collateral_amount != 0 {
        let withdraw_liquidity_amount =
            redeem_reserve_collateral(withdraw_reserve, withdraw_collateral_amount, clock, false)?;
        let protocol_fee = liquidation_operations::calculate_protocol_liquidation_fee(
            withdraw_liquidity_amount,
            liquidation_bonus_rate,
            withdraw_reserve.config.protocol_liquidation_fee_pct,
        );
        msg!(
            "pnl: Liquidator repaid {} and withdrew {} collateral with fees {}",
            repay_amount,
            withdraw_liquidity_amount.checked_sub(protocol_fee).unwrap(),
            protocol_fee
        );
        Ok(Some((withdraw_liquidity_amount, protocol_fee)))
    } else {
        Ok(None)
    }
}

pub fn flash_borrow_reserve_liquidity(reserve: &mut Reserve, liquidity_amount: u64) -> Result<()> {
    if reserve.config.fees.flash_loan_fee_sf == u64::MAX {
        msg!("Flash loans are disabled for this reserve");
        return err!(LendingError::FlashLoansDisabled);
    }

    let liquidity_amount_f = Fraction::from(liquidity_amount);

    reserve.liquidity.borrow(liquidity_amount_f)?;
    reserve.last_update.mark_stale();

    Ok(())
}

pub fn flash_repay_reserve_liquidity<'info>(
    reserve: &mut Reserve,
    liquidity_amount: u64,
    slot: Slot,
) -> Result<(u64, u64)> {
    let flash_loan_amount = liquidity_amount;

    let flash_loan_amount_f = Fraction::from(flash_loan_amount);
    let protocol_fee = reserve
        .config
        .fees
        .calculate_flash_loan_fees(flash_loan_amount_f)?;

    reserve
        .liquidity
        .repay(flash_loan_amount, flash_loan_amount_f)?;
    refresh_reserve_limit_timestamps(reserve, slot)?;
    reserve.last_update.mark_stale();

    Ok((flash_loan_amount, protocol_fee))
}

// Price utilities
pub fn is_saved_price_age_valid(reserve: &Reserve, current_ts: UnixTimestamp) -> bool {
    let current_ts: u64 = current_ts.try_into().expect("Negative timestamp");
    let price_last_updated_ts = reserve.liquidity.market_price_last_updated_ts;
    let price_max_age = reserve.config.token_info.max_age_price_seconds;

    current_ts.saturating_sub(price_last_updated_ts) < price_max_age
}

pub fn is_price_refresh_needed(
    reserve: &Reserve,
    market: &LendingMarket,
    current_ts: UnixTimestamp,
) -> bool {
    let current_ts = current_ts as u64;
    let price_last_updated_ts = reserve.liquidity.market_price_last_updated_ts;
    let price_max_age = reserve.config.token_info.max_age_price_seconds;
    let price_refresh_trigger_to_max_age_pct: u64 =
        market.price_refresh_trigger_to_max_age_pct.into();
    let price_refresh_trigger_to_max_age_secs =
        price_max_age * price_refresh_trigger_to_max_age_pct / 100;

    current_ts.saturating_sub(price_last_updated_ts) >= price_refresh_trigger_to_max_age_secs
}

pub mod utils {
    use super::*;
    use crate::{
        constants::{ten_pow, FULL_BPS, PROGRAM_VERSION},
        state::{ObligationCollateral, ObligationLiquidity, ReserveConfig},
        utils::FRACTION_ONE_SCALED,
    };

    pub(crate) fn repay_and_withdraw_from_obligation_post_liquidation(
        obligation: &mut Obligation,
        repay_reserve: &mut Reserve,
        settle_amount_f: Fraction,
        withdraw_amount: u64,
        repay_amount: u64,
        liquidity_index: usize,
        collateral_index: usize,
    ) -> Result<()> {
        if repay_amount == 0 {
            msg!("Liquidation is too small to transfer liquidity");
            return err!(LendingError::LiquidationTooSmall);
        }
        if withdraw_amount == 0 {
            msg!("Liquidation is too small to receive collateral");
            return err!(LendingError::LiquidationTooSmall);
        }

        repay_reserve
            .liquidity
            .repay(repay_amount, settle_amount_f)?;
        repay_reserve.last_update.mark_stale();

        obligation.repay(settle_amount_f, liquidity_index)?;
        obligation.withdraw(withdraw_amount, collateral_index)?;
        obligation.update_has_debt();
        obligation.last_update.mark_stale();

        Ok(())
    }

    pub(crate) fn calculate_market_value_from_liquidity_amount(
        reserve: &Reserve,
        liquidity_amount: Fraction,
    ) -> Result<Fraction> {
        let mint_decimal_factor: u128 =
            ten_pow(reserve.liquidity.mint_decimals.try_into().unwrap()).into();
        let market_price_f = reserve.liquidity.get_market_price_f();
        let market_value = liquidity_amount
            .mul(market_price_f)
            .div(mint_decimal_factor);

        Ok(market_value)
    }

    pub(crate) fn calculate_obligation_collateral_market_value(
        deposit_reserve: &Reserve,
        deposit: &ObligationCollateral,
    ) -> Result<Fraction> {
        let liquidity_amount_from_collateral = deposit_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(deposit.deposited_amount.into());

        calculate_market_value_from_liquidity_amount(
            deposit_reserve,
            liquidity_amount_from_collateral,
        )
    }

    pub(crate) fn calculate_obligation_liquidity_market_value(
        borrow_reserve: &Reserve,
        borrow: &ObligationLiquidity,
    ) -> Result<Fraction> {
        calculate_market_value_from_liquidity_amount(
            borrow_reserve,
            Fraction::from_bits(borrow.borrowed_amount_sf),
        )
    }

    pub(crate) fn check_obligation_collateral_deposit_reserve(
        deposit: &ObligationCollateral,
        deposit_reserve: &Reserve,
        deposit_reserve_pk: Pubkey,
        index: usize,
        slot: u64,
    ) -> Result<()> {
        if deposit.deposit_reserve != deposit_reserve_pk {
            msg!(
                "Deposit reserve of collateral {} does not match the deposit reserve provided",
                index
            );
            return err!(LendingError::InvalidAccountInput);
        }

        if deposit_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::NONE)?
        {
            msg!(
                "Deposit reserve {} provided for collateral {} is stale
                and must be refreshed in the current slot. Last Update {:?}",
                deposit.deposit_reserve,
                index,
                deposit_reserve.last_update,
            );
            return err!(LendingError::ReserveStale);
        }

        if deposit_reserve.version != PROGRAM_VERSION as u64 {
            msg!(
                "Deposit reserve {} provided for collateral {} has been deprecated.",
                deposit.deposit_reserve,
                index,
            );
            return err!(LendingError::ReserveDeprecated);
        }

        Ok(())
    }

    pub(crate) fn check_obligation_liquidity_borrow_reserve(
        borrow: &ObligationLiquidity,
        borrow_reserve: &Reserve,
        borrow_reserve_pk: Pubkey,
        index: usize,
        slot: u64,
    ) -> Result<()> {
        if borrow.borrow_reserve != borrow_reserve_pk {
            msg!(
                "Borrow reserve of liquidity {} does not match the borrow reserve provided",
                index
            );
            return err!(LendingError::InvalidAccountInput);
        }

        if borrow_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::NONE)?
        {
            msg!(
                "Borrow reserve {} provided for liquidity {} is stale
                and must be refreshed in the current slot. Last Update {:?}",
                borrow.borrow_reserve,
                index,
                borrow_reserve.last_update,
            );
            return err!(LendingError::ReserveStale);
        }

        if borrow_reserve.version != PROGRAM_VERSION as u64 {
            msg!(
                "Borrow reserve {} provided for liquidity {} has been deprecated.",
                borrow.borrow_reserve,
                index,
            );
            return err!(LendingError::ReserveDeprecated);
        }

        Ok(())
    }

    pub fn post_deposit_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        collateral_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf) + asset_mv;

        let new_collateral_asset_mv = collateral_asset_mv + asset_mv;

        let new_ltv = Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
            / new_total_deposited_mv;

        if new_collateral_asset_mv > 0 && new_collateral_asset_mv < min_accepted_net_value {
            msg!(
                "Obligation new collateral value after deposit {} for ${}",
                new_collateral_asset_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::NetValueRemainingTooSmall);
        }

        if obligation.deposited_value_sf != 0 {
            if new_ltv > obligation.loan_to_value() {
                msg!(
                    "Obligation new LTV after deposit {} of {}",
                    new_ltv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn post_withdraw_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        collateral_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf) - asset_mv;

        if collateral_asset_mv != 0 {
            let new_collateral_asset_mv = collateral_asset_mv - asset_mv;

            if new_collateral_asset_mv > 0 && new_collateral_asset_mv < min_accepted_net_value {
                msg!(
                    "Obligation new collateral value after withdraw {} for {}",
                    new_collateral_asset_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::NetValueRemainingTooSmall);
            }
        }

        if new_total_deposited_mv != 0 {
            if Fraction::from_bits(obligation.borrowed_assets_market_value_sf)
                >= new_total_deposited_mv
            {
                msg!(
                    "Obligation new total deposited market value after withdraw {} of {}",
                    new_total_deposited_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::LiabilitiesBiggerThanAssets);
            }

            let new_ltv = Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                / new_total_deposited_mv;

            let unhealthy_ltv = obligation.unhealthy_loan_to_value();

            if new_ltv > unhealthy_ltv {
                msg!(
                    "Obligation new LTV/new unhealthy LTV after withdraw {:.2}/{:.2} of {}",
                    new_ltv.to_display(),
                    unhealthy_ltv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn post_borrow_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        liquidity_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_bf_debt_mv =
            Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                + asset_mv * reserve.borrow_factor_f();
        let new_total_no_bf_debt_mv =
            Fraction::from_bits(obligation.borrowed_assets_market_value_sf) + asset_mv;
        let new_liquidity_asset_mv = liquidity_asset_mv + asset_mv;

        if new_liquidity_asset_mv > 0 && new_liquidity_asset_mv < min_accepted_net_value {
            msg!(
                "Obligation new borrowed value after borrow {} for {}",
                new_liquidity_asset_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::NetValueRemainingTooSmall);
        }
        let new_ltv = new_total_bf_debt_mv / Fraction::from_bits(obligation.deposited_value_sf);

        if new_ltv > obligation.unhealthy_loan_to_value() {
            msg!(
                "Obligation new LTV/new unhealthy LTV after borrow {:.2}/{:.2} of {}",
                new_ltv.to_display(),
                obligation.unhealthy_loan_to_value().to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::WorseLTVBlocked);
        }

        if new_total_no_bf_debt_mv >= Fraction::from_bits(obligation.deposited_value_sf) {
            msg!(
                "Obligation can't have more liabilities than assets after borrow {} of {}",
                new_total_no_bf_debt_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::LiabilitiesBiggerThanAssets);
        }

        Ok(())
    }

    pub fn post_repay_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        liquidity_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;
        let new_total_bf_debt_mv =
            Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                - asset_mv * reserve.borrow_factor_f();
        let total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf);

        if liquidity_asset_mv != 0 {
            let new_liquidity_asset_mv = liquidity_asset_mv - asset_mv;

            if new_liquidity_asset_mv > 0 && new_liquidity_asset_mv < min_accepted_net_value {
                msg!(
                    "Obligation new borrowed value after repay {} for {}",
                    new_liquidity_asset_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::NetValueRemainingTooSmall);
            }
        }
        if total_deposited_mv > 0 {
            let new_ltv = new_total_bf_debt_mv / total_deposited_mv;

            if new_ltv > obligation.loan_to_value() {
                msg!(
                    "Obligation new LTV/new unhealthy LTV after repay {:.2}/{:.2} of {}",
                    new_ltv.to_display(),
                    obligation.unhealthy_loan_to_value().to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn check_obligation_fully_refreshed_and_not_null(
        obligation: &Obligation,
        slot: Slot,
    ) -> Result<()> {
        if obligation
            .last_update
            .is_stale(slot, PriceStatusFlags::ALL_CHECKS)?
        {
            msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
            return err!(LendingError::ObligationStale);
        }
        if obligation.deposits_empty() {
            msg!("Obligation has no deposits to borrow against");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        if obligation.deposited_value_sf == 0 {
            msg!("Obligation deposits have zero value");
            return err!(LendingError::ObligationDepositsZero);
        }

        Ok(())
    }

    pub fn assert_obligation_liquidatable(
        repay_reserve: &Reserve,
        withdraw_reserve: &Reserve,
        obligation: &Obligation,
        liquidity_amount: u64,
        slot: Slot,
    ) -> Result<()> {
        if liquidity_amount == 0 {
            msg!("Liquidity amount provided cannot be zero");
            return err!(LendingError::InvalidAmount);
        }

        if repay_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
                "Repay reserve is stale and must be refreshed in the current slot, price status: {:08b}",
                repay_reserve.last_update.get_price_status().0
            );
            return err!(LendingError::ReserveStale);
        }

        if withdraw_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
                "Withdraw reserve is stale and must be refreshed in the current slot, price status: {:08b}",
                withdraw_reserve.last_update.get_price_status().0
            );
            return err!(LendingError::ReserveStale);
        }

        if obligation
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
            return err!(LendingError::ObligationStale);
        }

        if obligation.deposited_value_sf == 0 {
            msg!("Obligation deposited value is zero");
            return err!(LendingError::ObligationDepositsZero);
        }
        if obligation.borrow_factor_adjusted_debt_value_sf == 0 {
            msg!("Obligation borrowed value is zero");
            return err!(LendingError::ObligationBorrowsZero);
        }

        Ok(())
    }

    pub fn validate_reserve_config(config: &ReserveConfig) -> Result<()> {
        if config.loan_to_value_pct >= 100 {
            msg!("Loan to value ratio must be in range [0, 100)");
            return err!(LendingError::InvalidConfig);
        }
        if config.max_liquidation_bonus_bps > FULL_BPS {
            msg!("Liquidation bonus must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if config.liquidation_threshold_pct < config.loan_to_value_pct
            || config.liquidation_threshold_pct > 100
        {
            msg!("Liquidation threshold must be in range [LTV, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if u128::from(config.fees.borrow_fee_sf) >= FRACTION_ONE_SCALED {
            msg!("Borrow fee must be in range [0, 100%]");
            return err!(LendingError::InvalidConfig);
        }
        if config.protocol_liquidation_fee_pct > 100 {
            msg!("Protocol liquidation fee must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if config.protocol_take_rate_pct > 100 {
            msg!("Protocol take rate must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if !config.token_info.is_valid() {
            msg!("Invalid reserve token info");
            return err!(LendingError::InvalidOracleConfig);
        }
        if !config.token_info.is_twap_config_valid() {
            msg!("Invalid reserve token twap config");
            return err!(LendingError::InvalidTwapConfig);
        }

        if config.bad_debt_liquidation_bonus_bps >= 100 {
            msg!("Invalid bad debt liquidation bonus, cannot be more than 1%");
            return err!(LendingError::InvalidConfig);
        }
        if config.min_liquidation_bonus_bps > config.max_liquidation_bonus_bps {
            msg!("Invalid min liquidation bonus");
            return err!(LendingError::InvalidConfig);
        }
        if config.borrow_factor_pct < 100 {
            msg!("Invalid borrow factor, it must be greater or equal to 100");
            return err!(LendingError::InvalidConfig);
        }
        if config.deleveraging_threshold_slots_per_bps == 0 {
            msg!("Invalid deleveraging_threshold_slots_per_bps, must be greater than 0");
            return err!(LendingError::InvalidConfig);
        }

        config.borrow_rate_curve.validate()?;
        Ok(())
    }
}
