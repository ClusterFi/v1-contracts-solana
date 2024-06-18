use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use anchor_spl::token;
use anyhow::Result;
use cluster_lend::utils::pda::lending_market_auth;
use solana_program::{instruction::Instruction, sysvar};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, signature::Keypair, signer::Signer,
    transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

pub struct ObligationFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
}

impl ObligationFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        quote_currency: [u8; 32],
        account: &Keypair,
    ) -> Result<ObligationFixture, BanksClientError> {
        let ctx_ref = ctx.clone();
        let lending_market_authority = lending_market_auth(&lending_market);
 
        let mut ctx = ctx.borrow_mut();

        let accounts = cluster_lend::accounts::InitializeObligationCtx {
            owner: ctx.payer.pubkey(),
            lending_market,
            lending_market_authority,
            reserve: account.pubkey(),
            reserve_liquidity_mint,
            reserve_collateral_mint,
            reserve_liquidity_supply: pdas.liquidity_supply_vault,
            reserve_collateral_supply: pdas.collateral_supply_vault,
            fee_receiver: pdas.fee_vault,
            rent: rent::Rent::id(),
            token_program: token::ID,
            system_program: system_program::ID,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::InitializeObligation {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &account],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(ObligationFixture {
            ctx: ctx_ref,
            key: account.pubkey(),
            lending_market,
            config: ReserveConfig::default(),
            reserve_liquidity_mint,
            reserve_collateral_mint,
        })
    }
}
