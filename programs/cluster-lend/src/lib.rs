use anchor_lang::prelude::*;

mod processors;

use crate::processors::*;

declare_id!("E9Jcn8HfLEc9dG6VmPQqHhwqKEJNRUc1VLFhrvxdgkx9");

#[program]
pub mod cluster_lend {
    use super::*;

    pub fn init_market(ctx: Context<InitMarket>, quote_currency: [u8; 32]) -> Result<()> {
        process_init_market::handle(ctx, quote_currency)
    }

    pub fn update_market(
        ctx: Context<UpdateMarket>,
        mode: u64,
        value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
    ) -> Result<()> {
        process_update_market::handle(ctx, quote_currency)
    }
}

#[derive(Accounts)]
pub struct Initialize {}
