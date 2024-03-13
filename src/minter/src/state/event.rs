use crate::lifecycle::{InitArg, UpgradeArg};
use crate::state::{
    DepositEvent, InvalidSolTransaction, SkippedSolSignatureRange, SkippedSolTransaction,
};

use minicbor::{Decode, Encode};

/// The event describing the ckETH minter state transition.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum EventType {
    /// The minter initialization event.
    /// Must be the first event in the log.
    #[n(0)]
    Init(#[n(0)] InitArg),
    /// The minter upgraded with the specified arguments.
    #[n(1)]
    Upgrade(#[n(0)] UpgradeArg),
    /// The minter synced to the specified signature.
    #[n(2)]
    SyncedToSignature {
        /// The last processed signature (inclusive).
        #[n(0)]
        signature: String,
    },
    #[n(3)]
    SkippedSolSignatureRange {
        /// The skipped signature range in solana.
        #[n(0)]
        range: SkippedSolSignatureRange,
        /// The reason for skipping the range.
        #[n(1)]
        reason: String,
    },
    #[n(4)]
    SkippedSolTransaction {
        /// The skipped transaction.
        #[n(0)]
        sol_tx: SkippedSolTransaction,
        /// The reason for skipping the transaction in solana.
        #[n(1)]
        reason: String,
    },
    #[n(5)]
    InvalidDeposit {
        /// The invalid transaction.
        #[n(0)]
        sol_tx: InvalidSolTransaction,
        /// The reason for invalidating the transaction in solana.
        #[n(1)]
        reason: String,
    },
    #[n(6)]
    AcceptedDeposit {
        /// The accepted deposit.
        #[n(0)]
        deposit: DepositEvent,
        /// The signature of the deposit transaction in solana.
        #[n(1)]
        sol_sig: String,
    },
    #[n(7)]
    MintedDeposit {
        /// The minted ckETH event.
        #[n(0)]
        deposit: DepositEvent,
        /// The mint block index.
        #[n(1)]
        // TODO: is u64 enough?
        icp_mint_block_index: u64,
    },
}

#[derive(Encode, Decode, Debug, PartialEq, Eq)]
pub struct Event {
    /// The canister time at which the minter generated this event.
    #[n(0)]
    pub timestamp: u64,
    /// The event type.
    #[n(1)]
    pub payload: EventType,
}
