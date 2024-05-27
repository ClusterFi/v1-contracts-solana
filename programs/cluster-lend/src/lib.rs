use anchor_lang::prelude::*;

mod error;
mod processors;
mod state;
mod utils;

use error::*;
use processors::*;
use state::*;
use utils::*;

declare_id!("E9Jcn8HfLEc9dG6VmPQqHhwqKEJNRUc1VLFhrvxdgkx9");

#[program]
pub mod cluster_lend {
    use instruction::Repay;

    use super::*;

    pub fn initialize_market(
        ctx: Context<InitLendingMarket>,
        quote_currency: [u8; 32],
    ) -> Result<()> {
        init_lending_market::process(ctx, quote_currency)
    }

    pub fn update_market(
        ctx: Context<UpdateLendingMarket>,
        mode: u64,
        value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
    ) -> Result<()> {
        update_lending_market::process(ctx, mode, value)
    }

    pub fn deposit(ctx: Context<DepositCtx>) -> Result<()> {
        deposit::process(ctx)
    }

    pub fn borrow(ctx: Context<BorrowCtx>) -> Result<()> {
        borrow::process(ctx)
    }

    pub fn withdraw(ctx: Context<WithdrawCtx>) -> Result<()> {
        withdraw::process(ctx)
    }

    pub fn repay(ctx: Context<RepayCtx>) -> Result<()> {
        repay::process(ctx)
    }
}

#[derive(Accounts)]
pub struct Initialize {}
