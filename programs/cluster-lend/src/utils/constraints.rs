use anchor_lang::{err, prelude::AccountLoader, Result};

use crate::{errors::LendingError, state::LendingMarket};

pub fn emergency_mode_disabled(lending_market: &AccountLoader<LendingMarket>) -> Result<()> {
    if lending_market.load()?.emergency_mode > 0 {
        return err!(LendingError::GlobalEmergencyMode);
    }
    Ok(())
}
