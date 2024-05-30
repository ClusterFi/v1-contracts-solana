pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;
use instructions::*;

declare_id!("FtQFCy8pGnywDh1r2wZJWH8e5KHrkJvDzjTGv3LAAWmj");

#[program]
pub mod cluster_lend {
    use super::*;

    pub fn initialize_market(ctx: Context<InitializeMarketCtx>) -> Result<()> {
        Ok(())
    }
}
