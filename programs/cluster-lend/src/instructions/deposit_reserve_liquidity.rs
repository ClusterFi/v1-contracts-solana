use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::{self, Mint, Token, TokenAccount};

use crate::{
    constants::PROGRAM_VERSION,
    errors::LendingError,
    gen_signer_seeds,
    lending_market::refresh_reserve,
    state::{LendingAction, LendingMarket, Reserve, ReserveStatus},
    utils::{seeds, token_transfer},
};

pub fn process_deposit_reserve_liquidity(
    ctx: Context<DepositReserveLiquidityCtx>,
    liquidity_amount: u64,
) -> Result<()> {
    require!(liquidity_amount != 0, LendingError::InvalidAmount);

    let source_liquidity_info = &ctx.accounts.user_source_liquidity;
    let destination_collateral_info = &ctx.accounts.user_destination_collateral;
    let reserve_info = &ctx.accounts.reserve;
    let reserve_liquidity_supply_info = &ctx.accounts.reserve_liquidity_supply;
    let reserve_collateral_mint_info = &ctx.accounts.reserve_collateral_mint;
    let lending_market_info = &ctx.accounts.lending_market;
    let lending_market_authority_info = &ctx.accounts.lending_market_authority;
    let user_transfer_authority_info = &ctx.accounts.owner;
    let clock = &Clock::get()?;

    let lending_market = &mut lending_market_info.load()?;
    let reserve = &mut reserve_info.load_mut()?;

    if reserve.liquidity.supply_vault == source_liquidity_info.key() {
        msg!("Reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if reserve.collateral.supply_vault == destination_collateral_info.key() {
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

    refresh_reserve(reserve, &clock, None)?;

    let lending_market_key = ctx.accounts.lending_market.key();
    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump as u8);

    let initial_reserve_token_balance =
        token::accessor::amount(&ctx.accounts.reserve_liquidity_supply.to_account_info())?;
    let initial_reserve_available_liquidity = reserve.liquidity.available_amount;
    let collateral_amount =
        lending_operations::deposit_reserve_liquidity(reserve, &clock, liquidity_amount)?;

    msg!(
        "pnl: Depositing in reserve {:?} liquidity {}",
        ctx.accounts.reserve.key(),
        liquidity_amount
    );

    token_transfer::deposit_reserve_liquidity_transfer(
        ctx.accounts.user_source_liquidity.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.user_destination_collateral.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        liquidity_amount,
        collateral_amount,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token::accessor::amount(&ctx.accounts.reserve_liquidity_supply.to_account_info()).unwrap(),
        reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Additive(liquidity_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityCtx<'info> {
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut, address = reserve.load()?.liquidity.supply_vault)]
    pub reserve_liquidity_supply: Box<Account<'info, TokenAccount>>,

    #[account(mut, address = reserve.load()?.collateral.mint_pubkey)]
    pub reserve_collateral_mint: Box<Account<'info, Mint>>,

    #[account(mut,
        token::mint = reserve_liquidity_supply.mint
    )]
    pub user_source_liquidity: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        token::mint = reserve_collateral_mint.key()
    )]
    pub user_destination_collateral: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
