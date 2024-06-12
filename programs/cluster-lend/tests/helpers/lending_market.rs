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

pub struct LendingMarketFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
}

impl LendingMarketFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        quote_currency: [u8; 32],
        account: &Keypair,
    ) -> Result<LendingMarketFixture, BanksClientError> {
        let ctx_ref = ctx.clone();
        let lending_market_authority = lending_market_auth(&account.pubkey());

        let mut ctx = ctx.borrow_mut();

        let accounts = cluster_lend::accounts::InitializeMarketCtx {
            owner: ctx.payer.pubkey(),
            lending_market: account.pubkey(),
            lending_market_authority,
            system_program: system_program::ID,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::InitializeMarket { quote_currency }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &account],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(LendingMarketFixture {
            ctx: ctx_ref,
            key: account.pubkey(),
        })
    }

    pub async fn try_update_market(
        &self,
        owner: Keypair,
        mode: u64,
        value: [u8; 72],
    ) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::UpdateMarketCtx {
            owner: owner.pubkey(),
            lending_market: self.key,
            system_program: system_program::ID,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateMarket { mode, value }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &owner],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_update_market_owner(
        &self,
        owner: Keypair,
        new_owner: Pubkey,
    ) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::UpdateMarketOwnerCtx {
            owner: owner.pubkey(),
            lending_market: self.key,
            new_owner,
            system_program: system_program::ID,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateMarketOwner {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &owner],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }
}
