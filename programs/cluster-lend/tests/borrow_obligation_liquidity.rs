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
async fn success_init_reserve() {
}
