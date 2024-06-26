#[cfg(test)]
mod helpers;
use std::rc::Rc;

use anchor_lang::context;
use anchor_spl::{
    associated_token::{create, get_associated_token_address, AssociatedToken},
    token::{self, spl_token::state::Account, TokenAccount},
};
use cluster_lend::{
    constants::ten_pow, errors::LendingError, utils::pda, InitObligationArgs, LendingMarket,
    Reserve, ReserveStatus, UpdateLendingMarketMode,
};
use lending_market::LendingMarketFixture;

use obligation::ObligationFixture;
use reserve::ReserveFixture;
use solana_program_test::*;

use helpers::*;
use solana_sdk::{
    clock::{self, Clock},
    pubkey::Pubkey,
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

    let test_f = TestFixture::new().await;

    let payer = test_f.payer_keypair();

    let liquidity_mint = &test_f.usdc_mint;

    // create test user and supply test token
    let user = Keypair::new();
    let user_liquidity_ata = liquidity_mint
        .create_token_account_and_mint_to(&user, 1000)
        .await;

    // prepare market & reserve & obligation
    let lending_market_key = Keypair::new();
    let lending_market_f = LendingMarketFixture {
        key: lending_market_key.pubkey(),
        owner: payer.pubkey(),
    };

    let reserve_key = Keypair::new();
    let reserve_f = ReserveFixture {
        key: reserve_key.pubkey(),
        owner: payer.pubkey(),
        payer: payer.pubkey(),
        lending_market: lending_market_f.key,
        liquidity_mint: liquidity_mint.key,
    };

    let reserve_pdas = pda::init_reserve_pdas(&lending_market_f.key, &liquidity_mint.key);

    let init_obligation_args = InitObligationArgs { tag: 0, id: 0 };
    let obligation_key = pda::init_obligation_pda(
        &user.pubkey(),
        &lending_market_f.key,
        &Pubkey::default(),
        &Pubkey::default(),
        &init_obligation_args,
    );
    let obligation_f = ObligationFixture {
        key: obligation_key,
        owner: user.pubkey(),
        payer: payer.pubkey(),
        lending_market: lending_market_f.key,
    };

    let r = test_f
        .send_transaction(
            &[
                lending_market_f.init_market_ix(USDC_QUOTE_CURRENCY),
                reserve_f.initialize_reserve_ix(),
                reserve_f.update_reserve_ix(TEST_RESERVE_CONFIG),
                reserve_f.refresh_ix(Some(PYTH_SOL_FEED)),
                obligation_f.initialize_obligation_ix(init_obligation_args),
                obligation_f.refresh_ix(),
            ],
            &[&payer, &user, &lending_market_key, &reserve_key],
        )
        .await;
    assert!(r.is_ok());

    // deposit obligation
    let deposit_amount = 1_000_000;
    let r = test_f
        .send_transaction(
            &[
                obligation_f.deposit_liquidity_collateral_ix(
                    deposit_amount,
                    &reserve_f,
                    user_liquidity_ata.key,
                ),
            ],
            &[&payer, &user],
        )
        .await;
    assert!(r.is_ok());

    // check user's balance
    let user_ata: TokenAccount = test_f.load_and_deserialize(&user_liquidity_ata.key).await;
    // assert_eq!(user_ata.amount, balance - liquidity_amount);

    /*
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
     */
}
