use anchor_lang::{prelude::*, Accounts};

use crate::state::LendingMarket;

pub fn process(ctx: Context<UpdateLendingMarketOwner>) -> Result<()> {
    let market = &mut ctx.accounts.lending_market.load_mut()?;

    market.owner = market.owner_cached;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarketOwner<'info> {
    owner_cached: Signer<'info>,

    #[account(mut, has_one = owner_cached)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
