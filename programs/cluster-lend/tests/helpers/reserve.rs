use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use anchor_spl::token::{self, spl_token::instruction, Token};
use anyhow::Result;
use cluster_lend::{
    constants::VALUE_BYTE_ARRAY_LEN_RESERVE,
    utils::pda::{init_reserve_pdas_program_id, lending_market_auth, InitReservePdas},
    ReserveConfig,
};
use solana_program::instruction::Instruction;
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    rent,
    signature::Keypair,
    signer::Signer,
    sysvar::{id, instructions, SysvarId},
    transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

use crate::spl::MintFixture;

pub struct ReserveFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
    pub lending_market: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub config: ReserveConfig,
}

impl ReserveFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        lending_market: Pubkey,
        reserve_liquidity_mint: Pubkey,
        account: &Keypair,
    ) -> Result<ReserveFixture, BanksClientError> {
        let ctx_ref = ctx.clone();
        let lending_market_authority = lending_market_auth(&lending_market);
        let pdas = init_reserve_pdas_program_id(
            &cluster_lend::ID,
            &lending_market,
            &reserve_liquidity_mint,
        );
        let reserve_collateral_mint = pdas.collateral_ctoken_mint;

        let mut ctx = ctx.borrow_mut();

        let accounts = cluster_lend::accounts::InitializeReserveCtx {
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
            data: cluster_lend::instruction::InitializeReserve {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &account],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        Ok(ReserveFixture {
            ctx: ctx_ref,
            key: account.pubkey(),
            lending_market,
            config: ReserveConfig::default(),
            reserve_liquidity_mint,
            reserve_collateral_mint,
        })
    }

    pub async fn try_refresh_reserve(&self, pyth_oracle: Pubkey) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::RefreshReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            pyth_oracle: Some(pyth_oracle),
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::RefreshReserve {}.data(),
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

    pub async fn try_update_reserve_mode(
        &self,
        owner: Keypair,
        mode: u64,
        value: [u8; 32],
    ) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::UpdateReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: owner.pubkey(),
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateReserveMode { mode, value }.data(),
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

    pub async fn try_update_reserve(
        &mut self,
        owner: Keypair,
        config: ReserveConfig,
    ) -> Result<(), BanksClientError> {
        let mut value = [0; VALUE_BYTE_ARRAY_LEN_RESERVE];
        let data = borsh::BorshSerialize::try_to_vec(&config).unwrap();
        value.copy_from_slice(data.as_slice());

        let mut ctx = self.ctx.borrow_mut();
        let accounts = cluster_lend::accounts::UpdateReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: owner.pubkey(),
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateReserve { value }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &owner],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;

        self.config = config;

        Ok(())
    }

    pub async fn try_deposit(
        &self,
        owner: &Keypair,
        user_source_liquidity: Pubkey,
        user_destination_collateral: Pubkey,
        liquidity_amount: u64,
    ) -> Result<(), BanksClientError> {
        let mut ctx = self.ctx.borrow_mut();
        let lending_market_authority = lending_market_auth(&self.lending_market);

        let pdas = init_reserve_pdas_program_id(
            &cluster_lend::ID,
            &self.lending_market,
            &self.reserve_liquidity_mint,
        );

        let accounts = cluster_lend::accounts::DepositReserveLiquidityCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: owner.pubkey(),
            lending_market_authority,
            reserve_collateral_mint: pdas.collateral_ctoken_mint,
            reserve_liquidity_supply: pdas.liquidity_supply_vault,
            user_source_liquidity,
            user_destination_collateral,
            token_program: Token::id(),
            instruction_sysvar_account: instructions::id(),
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::DepositReserveLiquidity { liquidity_amount }.data(),
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
