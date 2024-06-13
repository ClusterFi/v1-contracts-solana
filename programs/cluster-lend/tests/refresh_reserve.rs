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
async fn success_refresh_reserve() {
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

    let r = reserve_f.try_refresh_reserve().await;
    assert!(r.is_ok());
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

    let r = reserve_f.try_refresh_reserve().await;
    assert!(r.is_err());
    assert_custom_error!(r.unwrap_err(), LendingError::InvalidOracleConfig);
}
