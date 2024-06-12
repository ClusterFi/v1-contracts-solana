#[cfg(test)]
mod helpers;
use std::rc::Rc;

use cluster_lend::LendingMarket;
use lending_market::LendingMarketFixture;

use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    signature::Keypair,
};
use test::{TestFixture, SOL_QUOTE_CURRENCY, USDC_QUOTE_CURRENCY};

#[tokio::test]
async fn success_init_lending_market() {
    let test_f = TestFixture::new().await;

    let lending_market_key = Keypair::new();
    let lending_market_f = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &lending_market_key,
    )
    .await
    .unwrap();

    // Fetch & deserialize lending_market account
    let lending_market: LendingMarket = test_f.load_and_deserialize(&lending_market_f.key).await;

    // Check properties
    assert_eq!(lending_market.quote_currency, USDC_QUOTE_CURRENCY);
    assert_eq!(lending_market.owner, test_f.payer());
}

#[tokio::test]
async fn failure_init_lending_market_with_same_currency() {
    let test_f = TestFixture::new().await;

    let lending_market_key = Keypair::new();

    let r = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &lending_market_key,
    )
    .await;
    assert!(r.is_ok());

    // Try to init market with same key
    let r = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        SOL_QUOTE_CURRENCY,
        &lending_market_key,
    )
    .await;
    assert!(r.is_err());
}
