use anchor_lang::prelude::*;

pub fn initialize_market(ctx: Context<InitializeMarketCtx>) -> Result<()> {
    let market = &mut ctx.accounts.market.load_init()?;

    market.set_initial_configuration(ctx.accounts.admin.key());

    // emit!(MarginfiGroupCreateEvent {
    //     header: GroupEventHeader {
    //         marginfi_group: ctx.accounts.marginfi_group.key(),
    //         signer: Some(*ctx.accounts.admin.key)
    //     },
    // });

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeMarketCtx<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + std::mem::size_of::<LendingMarket>(),
    )]
    pub market: AccountLoader<'info, LendingMarket>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}
