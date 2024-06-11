use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use anchor_spl::token;
use solana_program::{instruction::Instruction, sysvar};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, signature::Keypair, signer::Signer,
    transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

pub struct LendingMarketFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
}

impl LendingMarketFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        quote_currency: [u8; 32],
    ) -> LendingMarketFixture {
        let ctx_ref = ctx.clone();
        let account_key = Keypair::new();

        {
            let mut ctx = ctx.borrow_mut();

            let accounts = cluster_lend::accounts::InitializeMarketCtx {
                owner: ctx.payer(),
                lending_market: lending_market_key.pubkey(),
                lending_market_authority: ctx.authority(),
                system_program: system_program::ID,
            };
            let init_marginfi_account_ix = Instruction {
                program_id: cluster_lend::id(),
                accounts: accounts.to_account_metas(Some(true)),
                data: cluster_lend::instruction::InitializeMarket { quote_currency }.data(),
            };

            let tx = Transaction::new_signed_with_payer(
                &[init_marginfi_account_ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer, &account_key],
                ctx.last_blockhash,
            );
            ctx.banks_client.process_transaction(tx).await.unwrap();
        }

        LendingMarketFixture {
            ctx: ctx_ref,
            key: account_key.pubkey(),
        }
    }
}
