use anchor_lang::prelude::*;

#[error_code]
pub enum LendingError {
    /// The account cannot be initialized because it is already in use.
    #[msg("Account is already initialized")]
    AlreadyInitialized,
    /// Lamport balance below rent-exempt threshold.
    #[msg("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    /// The program address provided doesn't match the value generated by the
    /// program.
    #[msg("Market authority is invalid")]
    InvalidMarketAuthority,
    /// Expected a different market owner
    #[msg("Market owner is invalid")]
    InvalidMarketOwner,

    // 5
    /// The owner of the input isn't set to the program address generated by the
    /// program.
    #[msg("Input account owner is not the program address")]
    InvalidAccountOwner,
    /// The owner of the account input isn't set to the correct token program
    /// id.
    #[msg("Input token account is not owned by the correct token program id")]
    InvalidTokenOwner,
    /// Expected an SPL Token account
    #[msg("Input token account is not valid")]
    InvalidTokenAccount,
    /// Expected an SPL Token mint
    #[msg("Input token mint account is not valid")]
    InvalidTokenMint,
    /// Expected a different SPL Token program
    #[msg("Input token program account is not valid")]
    InvalidTokenProgram,

    // 10
    /// Invalid amount, must be greater than zero
    #[msg("Input amount is invalid")]
    InvalidAmount,
    /// Invalid config value
    #[msg("Input config value is invalid")]
    InvalidConfig,
    /// Invalid config value
    #[msg("Input account must be a signer")]
    InvalidSigner,
    /// Invalid account input
    #[msg("Invalid account input")]
    InvalidAccountInput,
    /// Math operation overflow
    #[msg("Math operation overflow")]
    MathOverflow,
    #[msg("Conversion between integers failed")]
    IntegerOverflow,

    // 15
    /// Token initialize mint failed
    #[msg("Token initialize mint failed")]
    TokenInitializeMintFailed,
    /// Token initialize account failed
    #[msg("Token initialize account failed")]
    TokenInitializeAccountFailed,
    /// Token transfer failed
    #[msg("Token transfer failed")]
    TokenTransferFailed,
    /// Token mint to failed
    #[msg("Token mint to failed")]
    TokenMintToFailed,
    /// Token burn failed
    #[msg("Token burn failed")]
    TokenBurnFailed,

    // 20
    /// Insufficient liquidity available
    #[msg("Insufficient liquidity available")]
    InsufficientLiquidity,
    /// This reserve's collateral cannot be used for borrows
    #[msg("Input reserve has collateral disabled")]
    ReserveCollateralDisabled,
    /// Reserve state stale
    #[msg("Reserve state needs to be refreshed")]
    ReserveStale,
    /// Withdraw amount too small
    #[msg("Withdraw amount too small")]
    WithdrawTooSmall,
    /// Withdraw amount too large
    #[msg("Withdraw amount too large")]
    WithdrawTooLarge,

    // 25
    /// Borrow amount too small
    #[msg("Borrow amount too small to receive liquidity after fees")]
    BorrowTooSmall,
    /// Borrow amount too large
    #[msg("Borrow amount too large for deposited collateral")]
    BorrowTooLarge,
    /// Repay amount too small
    #[msg("Repay amount too small to transfer liquidity")]
    RepayTooSmall,
    /// Liquidation amount too small
    #[msg("Liquidation amount too small to receive collateral")]
    LiquidationTooSmall,
    /// Cannot liquidate healthy obligations
    #[msg("Cannot liquidate healthy obligations")]
    ObligationHealthy,

    // 30
    /// Obligation state stale
    #[msg("Obligation state needs to be refreshed")]
    ObligationStale,
    /// Obligation reserve limit exceeded
    #[msg("Obligation reserve limit exceeded")]
    ObligationReserveLimit,
    /// Expected a different obligation owner
    #[msg("Obligation owner is invalid")]
    InvalidObligationOwner,
    /// Obligation deposits are empty
    #[msg("Obligation deposits are empty")]
    ObligationDepositsEmpty,
    /// Obligation borrows are empty
    #[msg("Obligation borrows are empty")]
    ObligationBorrowsEmpty,

    // 35
    /// Obligation deposits have zero value
    #[msg("Obligation deposits have zero value")]
    ObligationDepositsZero,
    /// Obligation borrows have zero value
    #[msg("Obligation borrows have zero value")]
    ObligationBorrowsZero,
    /// Invalid obligation collateral
    #[msg("Invalid obligation collateral")]
    InvalidObligationCollateral,
    /// Invalid obligation liquidity
    #[msg("Invalid obligation liquidity")]
    InvalidObligationLiquidity,
    /// Obligation collateral is empty
    #[msg("Obligation collateral is empty")]
    ObligationCollateralEmpty,

    #[msg("Obligation liquidity is empty")]
    ObligationLiquidityEmpty,
    #[msg("Interest rate is negative")]
    NegativeInterestRate,
    #[msg("Input oracle config is invalid")]
    InvalidOracleConfig,
    #[msg("Input flash loan receiver program account is not valid")]
    InvalidFlashLoanReceiverProgram,
    #[msg("Not enough liquidity after flash loan")]
    NotEnoughLiquidityAfterFlashLoan,
    #[msg("Amount smaller than desired slippage limit")]
    ExceededSlippage,
}

impl From<LendingError> for ProgramError {
    fn from(e: LendingError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
