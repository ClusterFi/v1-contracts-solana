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

    let collateral_mint = test_f.usdc_mint.key;
    let borrow_mint = test_f.sol_mint.key;

    // create test user and supply test token
    let depositor = Keypair::new();
    let depositor_ata_f = test_f
        .usdc_mint
        .create_token_account_and_mint_to(&depositor, 1000)
        .await;
    let balance = 1000 * ten_pow(USDC_MINT_DECIMALS as usize);

    let user_destination_collateral = TokenAccountFixture::new_with_keypair(
        test_f.context.clone(),
        &collateral_mint,
        &depositor.pubkey(),
        &Keypair::new(),
    )
    .await
    .key;

    // prepare market & reserve
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
        liquidity_mint: collateral_mint,
    };

    let reserve_pdas = pda::init_reserve_pdas(&lending_market_f.key, &collateral_mint);

    let r = test_f
        .send_transaction(
            &[
                lending_market_f.init_market_ix(USDC_QUOTE_CURRENCY),
                reserve_f.initialize_reserve_ix(),
                reserve_f.update_reserve_ix(TEST_RESERVE_CONFIG),
            ],
            &[&payer, &lending_market_key, &reserve_key],
        )
        .await;
    assert!(r.is_ok());

    // deposit token
    let liquidity_amount = 1_000;
    let user_source_liquidity =
        get_associated_token_address(&depositor.pubkey(), &reserve_pdas.collateral_ctoken_mint);
    let user_destination_collateral =
        get_associated_token_address(&depositor.pubkey(), &collateral_mint);

    let r = test_f
        .send_transaction(
            &[reserve_f.deposit_reserve_ix(
                liquidity_amount,
                user_source_liquidity,
                user_destination_collateral,
            )],
            &[&payer, &depositor],
        )
        .await;
    assert!(r.is_ok());

    // check user's balance
    let user_ata: TokenAccount = test_f.load_and_deserialize(&depositor_ata_f.key).await;
    assert_eq!(user_ata.amount, balance - liquidity_amount);

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
