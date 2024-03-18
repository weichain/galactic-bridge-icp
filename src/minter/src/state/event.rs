use crate::lifecycle::{InitArg, UpgradeArg};
use crate::state::{ReceivedSolEvent, SolanaSignature, SolanaSignatureRange};

use minicbor::{Decode, Encode};

/// The event describing the ckSol minter state transition.
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub enum EventType {
    /// The minter initialization event.
    /// Must be the first event in the log.
    #[n(0)]
    Init(#[n(0)] InitArg),
    /// The minter upgraded with the specified arguments.
    #[n(1)]
    Upgrade(#[n(0)] UpgradeArg),
    /// Last known signature by the minter.
    #[n(2)]
    LastKnownSolanaSignature(#[n(0)] String),
    /// New signature range in solana.
    #[n(3)]
    NewSolanaSignatureRange(#[n(0)] SolanaSignatureRange),
    #[n(4)]
    RemoveSolanaSignatureRange(#[n(0)] SolanaSignatureRange),
    #[n(5)]
    RetrySolanaSignatureRange {
        /// The previously failed range.
        #[n(0)]
        range: SolanaSignatureRange,
        /// A failed sub-range of the previously failed range.
        #[n(1)]
        failed_sub_range: Option<SolanaSignatureRange>,
        /// The reason for failure.
        #[n(2)]
        fail_reason: String,
    },
    #[n(6)]
    SolanaSignature {
        /// The skipped transaction.
        #[n(0)]
        signature: SolanaSignature,
        /// The reason for skipping the transaction in solana.
        #[n(1)]
        fail_reason: Option<String>,
    },
    #[n(7)]
    InvalidEvent {
        /// The invalid transaction.
        #[n(0)]
        signature: SolanaSignature,
        /// The reason for invalidating the transaction in solana.
        #[n(1)]
        fail_reason: String,
    },
    #[n(8)]
    AcceptedEvent {
        /// The accepted ReceivedSolEvent.
        #[n(0)]
        event_source: ReceivedSolEvent,
        /// The reason for failure.
        #[n(1)]
        fail_reason: Option<String>,
    },
    #[n(9)]
    MintedEvent {
        /// The minted ckSol event.
        #[n(0)]
        event_source: ReceivedSolEvent,
        #[n(2)]
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
