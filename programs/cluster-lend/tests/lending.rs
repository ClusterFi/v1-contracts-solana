#[cfg(test)]
mod helpers;
use std::rc::Rc;

use anchor_lang::context;
use anchor_spl::{
    associated_token::{create, get_associated_token_address, AssociatedToken},
    token::{self, spl_token::state::Account, TokenAccount},
};
use cluster_lend::{
    constants::ten_pow, errors::LendingError, InitObligationArgs, LendingMarket, Reserve,
    ReserveStatus, UpdateLendingMarketMode,
};
use lending_market::LendingMarketFixture;

use obligation::ObligationFixture;
use reserve::ReserveFixture;
use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    signature::Keypair,
    signer::Signer,
};
use spl::TokenAccountFixture;
use test::{
    TestFixture, PYTH_SOL_FEED, SOL_MINT_DECIMALS, SOL_QUOTE_CURRENCY, TEST_RESERVE_CONFIG,
    USDC_MINT_DECIMALS, USDC_QUOTE_CURRENCY,
};

#[tokio::test]
async fn success_lending() {
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

    let mut reserve_f = ReserveFixture::new(
        Rc::clone(&test_f.context),
        lending_market_f.key,
        test_f.usdc_mint.key,
        &Keypair::new(),
    )
    .await
    .unwrap();

    reserve_f
        .try_update_reserve(test_f.payer_keypair(), TEST_RESERVE_CONFIG)
        .await
        .unwrap();

    // create test user and supply test token
    let depositor = Keypair::new();
    let depositor_ata_f = test_f
        .usdc_mint
        .create_token_account_and_mint_to(&depositor, 1000)
        .await;
    let balance = 1000 * ten_pow(USDC_MINT_DECIMALS as usize);

    let user_destination_collateral = TokenAccountFixture::new_with_keypair(
        test_f.context.clone(),
        &reserve_f.reserve_collateral_mint,
        &depositor.pubkey(),
        &Keypair::new(),
    )
    .await
    .key;

    // deposit token
    let liquidity_amount = 1_000;
    let r = reserve_f
        .try_deposit(
            &depositor,
            depositor_ata_f.key,
            user_destination_collateral,
            liquidity_amount,
        )
        .await;
    assert!(r.is_ok());

    // check user's balance
    let user_ata: TokenAccount = test_f.load_and_deserialize(&depositor_ata_f.key).await;
    assert_eq!(user_ata.amount, balance - liquidity_amount);

    // refresh reserve
    let r = reserve_f.try_refresh_reserve(PYTH_SOL_FEED).await;
    assert!(r.is_ok());

    // init obligation
    let borrower = Keypair::new();
    let obligation_f = ObligationFixture::new(
        test_f.context.clone(),
        reserve_f,
        InitObligationArgs { id: 1, tag: 0 },
        &borrower,
    )
    .await
    .unwrap();

    // refresh obligation
    let r = obligation_f.try_refresh_obligation().await;
    assert!(r.is_ok());

    // withdraw token
}
