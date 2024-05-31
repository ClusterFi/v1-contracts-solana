pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;
use constants::VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE;
use instructions::*;

declare_id!("FtQFCy8pGnywDh1r2wZJWH8e5KHrkJvDzjTGv3LAAWmj");

#[program]
pub mod cluster_lend {

    use super::*;

    // Market instructions
    pub fn initialize_market(
        ctx: Context<InitializeMarketCtx>,
        quote_currency: [u8; 32],
    ) -> Result<()> {
        process_initialize_market(ctx, quote_currency)
    }

    pub fn update_market(
        ctx: Context<UpdateMarketCtx>,
        mode: u64,
        value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
    ) -> Result<()> {
        process_update_market(ctx, mode, value)
    }

    pub fn update_market_owner(ctx: Context<UpdateMarketOwnerCtx>) -> Result<()> {
        process_update_market_owner(ctx)
    }

    pub fn initialize_reserve<'info>(
        ctx: Context<'_, '_, '_, 'info, InitializeReserveCtx<'info>>,
    ) -> Result<()> {
        process_initialize_reserve(ctx)
    }

    // User instructions
    #[access_control(emergency_mode_disabled(&ctx.accounts.lending_market))]
    pub fn deposit_reserve_liquidity(
        ctx: Context<DepositReserveLiquidityCtx>,
        liquidity_amount: u64,
    ) -> Result<()> {
        process_deposit_reserve_liquidity(ctx, liquidity_amount)
    }
}
