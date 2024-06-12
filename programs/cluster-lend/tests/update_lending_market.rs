#[cfg(test)]
mod helpers;
use std::rc::Rc;

use cluster_lend::{LendingMarket, UpdateLendingMarketMode};
use lending_market::LendingMarketFixture;

use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    signature::Keypair,
    signer::Signer,
};
use test::{TestFixture, SOL_QUOTE_CURRENCY, USDC_QUOTE_CURRENCY};

#[tokio::test]
async fn success_update_lending_market() {
    let test_f = TestFixture::new().await;

    let lending_market_f = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &Keypair::new(),
    )
    .await
    .unwrap();

    let owner = test_f.payer_keypair();

    // update emergancy mode
    let mode = UpdateLendingMarketMode::UpdateEmergencyMode as u64;
    let mut value: [u8; 72] = [0; 72];
    value[0] = 1;
    let r = lending_market_f.try_update_market(owner, mode, value).await;
    assert!(r.is_ok());

    // Fetch & deserialize lending_market account
    let lending_market: LendingMarket = test_f.load_and_deserialize(&lending_market_f.key).await;

    // Check properties
    assert_eq!(lending_market.emergency_mode, 1);
}

#[tokio::test]
async fn success_update_lending_market_owner() {
    let test_f = TestFixture::new().await;

    let lending_market_f = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &Keypair::new(),
    )
    .await
    .unwrap();

    let owner = test_f.payer_keypair();
    let new_owner = Keypair::new();
    let r = lending_market_f
        .try_update_market_owner(owner, new_owner.pubkey())
        .await;
    assert!(r.is_ok());

    // Fetch & deserialize lending_market account
    let lending_market: LendingMarket = test_f.load_and_deserialize(&lending_market_f.key).await;

    // Check properties
    assert_eq!(lending_market.quote_currency, USDC_QUOTE_CURRENCY);
    assert_eq!(lending_market.owner, new_owner.pubkey());
}

#[tokio::test]
async fn failure_update_lending_market_with_invalid_owner() {
    let test_f = TestFixture::new().await;

    let lending_market_f = LendingMarketFixture::new(
        Rc::clone(&test_f.context),
        USDC_QUOTE_CURRENCY,
        &Keypair::new(),
    )
    .await
    .unwrap();

    let owner = Keypair::new();

    // update configure with invalid authority
    let mode = UpdateLendingMarketMode::UpdateBorrowingDisabled as u64;
    let mut value: [u8; 72] = [0; 72];
    value[0] = 1;
    let r = lending_market_f.try_update_market(owner, mode, value).await;
    assert!(r.is_err());
}
