use crate::state::{CalculateBorrowResult, Obligation, ReserveStatus};
use crate::utils::fraction::Fraction;
use crate::utils::BigFraction;
use crate::{
    errors::LendingError,
    state::{LendingMarket, PriceStatusFlags, Reserve},
    utils::GetPriceResult,
};
use anchor_lang::solana_program::clock::Slot;
use anchor_lang::{prelude::*, solana_program::clock::UnixTimestamp};

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
        let (reserve_loan_to_value_pct, _) = get_max_ltv_and_liquidation_threshold(
            lending_market,
            withdraw_reserve,
            obligation.elevation_group,
        )?;

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
