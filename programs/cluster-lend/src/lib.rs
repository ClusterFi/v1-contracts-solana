use anchor_lang::prelude::*;

mod processors;
mod state;
mod utils;

use processors::*;
use state::*;
use utils::*;

declare_id!("E9Jcn8HfLEc9dG6VmPQqHhwqKEJNRUc1VLFhrvxdgkx9");

#[program]
pub mod cluster_lend {
    use super::*;

    pub fn init_lending_market(
        ctx: Context<InitLendingMarket>,
        quote_currency: [u8; 32],
    ) -> Result<()> {
        init_lending_market::process(ctx, quote_currency)
    }

    pub fn update_lending_market(
        ctx: Context<UpdateLendingMarket>,
        mode: u64,
        value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
    ) -> Result<()> {
        update_lending_market::process(ctx, mode, value)
    }
}

#[derive(Accounts)]
pub struct Initialize {}
