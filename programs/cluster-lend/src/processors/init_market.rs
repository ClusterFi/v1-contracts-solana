use anchor_lang::{prelude::*, Accounts};

use crate::{
    state::{InitLendingMarketParams, LendingMarket},
    utils::seeds,
};

pub fn process(ctx: Context<InitMarketCtx>, quote_currency: [u8; 32]) -> Result<()> {
    let market = &mut ctx.accounts.market.load_init()?;

    market.init(InitMarketParams {
        quote_currency,
        owner: ctx.accounts.owner.key(),
        bump_seed: ctx.bumps.market_authority,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitMarketCtx<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(zero)]
    pub market: AccountLoader<'info, Market>,

    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, market.key().as_ref()],
        bump
    )]
    pub market_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
