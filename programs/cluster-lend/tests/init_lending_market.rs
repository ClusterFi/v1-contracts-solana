#[cfg(test)]
mod helpers;
use anchor_lang::prelude::Clock;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};

use solana_program::{instruction::Instruction, system_program};
use solana_program::{pubkey, pubkey::Pubkey};

use solana_program_test::*;

use helpers::*;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};
use test::TestFixture;

#[tokio::test]
async fn it_works() {
    let test_f = TestFixture::new().await;

    let lending_market_key = Keypair::new();
    let accounts = cluster_lend::accounts::InitializeMarketCtx {
        owner: test_f.payer(),
        lending_market: lending_market_key.pubkey(),
        lending_market_authority: test_f.authority.pubkey(),
        system_program: system_program::ID,
    };
    let init_marginfi_account_ix = Instruction {
        program_id: cluster_lend::id(),
        accounts: accounts.to_account_metas(Some(true)),
        data: cluster_lend::instruction::InitializeMarket {}.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_marginfi_account_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok());
}
