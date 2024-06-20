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
    pub key: Pubkey,
    pub owner: Pubkey,
    pub payer: Pubkey,
    pub lending_market: Pubkey,
    pub liquidity_mint: Pubkey,
}

impl ReserveFixture {
    pub fn initialize_reserve_ix(&self) -> Result<Instruction> {
        let lending_market_authority = lending_market_auth(&self.lending_market);
        let pdas = init_reserve_pdas_program_id(
            &cluster_lend::ID,
            &self.lending_market,
            &self.liquidity_mint,
        );

        let accounts = cluster_lend::accounts::InitializeReserveCtx {
            owner: self.owner,
            lending_market: self.lending_market,
            lending_market_authority,
            reserve: self.key,
            reserve_liquidity_mint: self.liquidity_mint,
            reserve_collateral_mint: pdas.collateral_ctoken_mint,
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

        Ok(ix)
    }

    pub fn update_reserve_ix(&self, config: ReserveConfig) -> Result<Instruction> {
        let mut value = [0; VALUE_BYTE_ARRAY_LEN_RESERVE];
        let data = borsh::BorshSerialize::try_to_vec(&config).unwrap();
        value.copy_from_slice(data.as_slice());

        let accounts = cluster_lend::accounts::UpdateReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: self.owner,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateReserve { value }.data(),
        };

        Ok(ix)
    }

    pub fn update_reserve_mode_ix(&self, mode: u64, value: [u8; 32]) -> Result<Instruction> {
        let accounts = cluster_lend::accounts::UpdateReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: self.owner,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::UpdateReserveMode { mode, value }.data(),
        };

        Ok(ix)
    }

    pub fn refresh_reserve_ix(&self, pyth_oracle: Option<Pubkey>) -> Result<Instruction> {
        let accounts = cluster_lend::accounts::RefreshReserveCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            pyth_oracle,
        };
        let ix = Instruction {
            program_id: cluster_lend::id(),
            accounts: accounts.to_account_metas(Some(true)),
            data: cluster_lend::instruction::RefreshReserve {}.data(),
        };

        Ok(ix)
    }

    pub fn deposit_reserve_ix(
        &self,
        liquidity_amount: u64,
        user_source_liquidity: Pubkey,
        user_destination_collateral: Pubkey,
    ) -> Result<Instruction> {
        let lending_market_authority = lending_market_auth(&self.lending_market);

        let pdas = init_reserve_pdas_program_id(
            &cluster_lend::ID,
            &self.lending_market,
            &self.liquidity_mint,
        );

        let accounts = cluster_lend::accounts::DepositReserveLiquidityCtx {
            reserve: self.key,
            lending_market: self.lending_market,
            owner: self.owner,
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

        Ok(ix)
    }
}
