use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use anchor_spl::token;
use anyhow::Result;
use cluster_lend::{utils::pda::lending_market_auth, InitObligationArgs};
use solana_program::{instruction::Instruction, sysvar};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, rent, signature::Keypair, signer::Signer,
    sysvar::SysvarId, transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

use crate::reserve::ReserveFixture;

pub struct ObligationFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub reserve: ReserveFixture,
    pub key: Pubkey,
}

impl ObligationFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        reserve: ReserveFixture,
        args: InitObligationArgs,
        account: &Keypair,
    ) -> Result<ObligationFixture, BanksClientError> {
        let ctx_ref = ctx.clone();

        let mut ctx = ctx.borrow_mut();

        let accounts = cluster_lend::accounts::InitializeObligationCtx {
            owner: ctx.payer.pubkey(),
            fee_payer: ctx.payer.pubkey(),
            lending_market: reserve.lending_market,
            obligation: account.pubkey(),
            seed1_account: Pubkey::default(),
            seed2_account: Pubkey::default(),
            rent: rent::Rent::id(),
            token_program: token::ID,
            system_program: system_program::ID,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::InitializeObligation { args }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(ObligationFixture {
            ctx: ctx_ref,
            key: account.pubkey(),
            reserve,
        })
    }

    pub async fn try_refresh_obligation(&self) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::RefreshObligationCtx {
            lending_market: self.reserve.lending_market,
            obligation: self.key,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::RefreshObligation {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }
}
