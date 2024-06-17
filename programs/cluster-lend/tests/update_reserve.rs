#[cfg(test)]
mod helpers;
use std::rc::Rc;

use cluster_lend::{LendingMarket, Reserve, ReserveStatus, UpdateConfigMode, UpdateLendingMarketMode};
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
async fn success_update_reserve() {
    let test_f = TestFixture::new().await;

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

    reserve_f.try_update_reserve(test_f.payer_keypair(), );

    // Fetch reserve account
    let reserve: Reserve = test_f.load_and_deserialize(&reserve_f.key).await;

    // Check properties
    assert_eq!(reserve.lending_market, lending_market_key.pubkey());
    assert_eq!(reserve.config.status(), ReserveStatus::Hidden);
}