#[cfg(test)]
mod helpers;
use std::rc::Rc;

use cluster_lend::{
    errors::LendingError, LendingMarket, Reserve, ReserveStatus, UpdateLendingMarketMode,
};
use lending_market::LendingMarketFixture;

use reserve::ReserveFixture;
use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    signature::Keypair,
    signer::Signer,
};
use test::{TestFixture, SOL_MINT_DECIMALS, SOL_QUOTE_CURRENCY, USDC_QUOTE_CURRENCY};

#[tokio::test]
async fn success_deposit() {
    // Create market & reserve
    let mut test_f = TestFixture::new().await;

    let lending_market_key = Keypair::new();
    let lending_market_f = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &lending_market_key,
    )
    .await
    .unwrap();

    let reserve_key = Keypair::new();
    let reserve_f = ReserveFixture::new(
        Rc::clone(&test_f.context),
        lending_market_f.key,
        test_f.usdc_mint.key,
        &reserve_key,
    )
    .await
    .unwrap();

    // create test user and supply test token
    let user = Keypair::new();
    test_f.usdc_mint.key = user.pubkey();
    test_f.usdc_mint.create_token_account_and_mint_to(100).await;

    // deposit


}

#[tokio::test]
async fn success_withdraw() {}
