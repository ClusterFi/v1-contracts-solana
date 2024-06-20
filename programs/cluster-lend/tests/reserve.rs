#[cfg(test)]
mod helpers;
use std::rc::Rc;

use anchor_lang::AnchorSerialize;
use cluster_lend::{
    errors::LendingError, LendingMarket, Reserve, ReserveStatus, UpdateConfigMode,
    UpdateLendingMarketMode,
};
use lending_market::LendingMarketFixture;

use reserve::ReserveFixture;
use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};
use test::{
    TestFixture, PYTH_SOL_FEED, SOL_MINT_DECIMALS, SOL_QUOTE_CURRENCY, TEST_RESERVE_CONFIG,
    USDC_QUOTE_CURRENCY,
};

#[tokio::test]
async fn success_init_update_reserve() {
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
    let mut reserve_f = ReserveFixture::new(
        Rc::clone(&test_f.context),
        lending_market_f.key,
        test_f.usdc_mint.key,
        &reserve_key,
    )
    .await
    .unwrap();

    // Fetch reserve account
    let reserve: Reserve = test_f.load_and_deserialize(&reserve_f.key).await;

    // Check properties
    assert_eq!(reserve.lending_market, lending_market_key.pubkey());
    assert_eq!(reserve.config.status(), ReserveStatus::Hidden);

    // Test as entire config update
    let r = reserve_f
        .try_update_reserve(test_f.payer_keypair(), TEST_RESERVE_CONFIG)
        .await;
    assert!(r.is_ok(), "Update reserve failed");

    let r = reserve_f.try_refresh_reserve(PYTH_SOL_FEED).await;
    assert!(r.is_ok());

    let reserve: Reserve = test_f.load_and_deserialize(&reserve_f.key).await;
    assert_eq!(reserve.config.status(), ReserveStatus::Active);
    assert_eq!(
        reserve.config.deleveraging_margin_call_period_secs,
        TEST_RESERVE_CONFIG.deleveraging_margin_call_period_secs
    );

    // Test as individual field
    let mut value: [u8; 32] = [0; 32];
    value[0] = 32;
    let r = reserve_f
        .try_update_reserve_mode(
            test_f.payer_keypair(),
            UpdateConfigMode::UpdateLoanToValuePct as u64,
            value,
        )
        .await;
    assert!(r.is_ok(), "Update reserve failed");

    let r = reserve_f.try_refresh_reserve(PYTH_SOL_FEED).await;
    assert!(r.is_ok());

    let reserve: Reserve = test_f.load_and_deserialize(&reserve_f.key).await;
    assert_eq!(reserve.config.loan_to_value_pct, 32);
}

#[tokio::test]
async fn failure_refresh_reserve_invalid_oracle() {
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

    let r = reserve_f.try_refresh_reserve(Pubkey::default()).await;
    assert!(r.is_err());
    assert_custom_error!(r.unwrap_err(), LendingError::InvalidOracleConfig);
}
