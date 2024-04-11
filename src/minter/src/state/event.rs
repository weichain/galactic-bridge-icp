use crate::lifecycle::{InitArg, UpgradeArg};
use crate::state::{DepositEvent, SolanaSignature, SolanaSignatureRange, WithdrawalEvent};

use minicbor::{Decode, Encode};

/// The event describing the gSol minter state transition.
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
    #[n(3)]
    LastDepositIdCounter(#[n(0)] u64),
    #[n(4)]
    LastBurnIdCounter(#[n(0)] u64),
    /// New signature range in solana.
    #[n(5)]
    NewSolanaSignatureRange(#[n(0)] SolanaSignatureRange),
    #[n(6)]
    RemoveSolanaSignatureRange(#[n(0)] SolanaSignatureRange),
    #[n(7)]
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
    #[n(8)]
    SolanaSignature {
        /// The skipped transaction.
        #[n(0)]
        signature: SolanaSignature,
        /// The reason for skipping the transaction in solana.
        #[n(1)]
        fail_reason: Option<String>,
    },
    #[n(9)]
    InvalidEvent {
        /// The invalid transaction.
        #[n(0)]
        signature: SolanaSignature,
        /// The reason for invalidating the transaction in solana.
        #[n(1)]
        fail_reason: String,
    },
    #[n(10)]
    AcceptedEvent {
        /// The accepted DepositEvent.
        #[n(0)]
        event_source: DepositEvent,
        /// The reason for failure.
        #[n(1)]
        fail_reason: Option<String>,
    },
    #[n(11)]
    MintedEvent {
        /// The minted gSol event.
        #[n(0)]
        event_source: DepositEvent,
    },
    #[n(12)]
    WithdrawalBurnedEvent {
        /// The withdrawal gSOL burned event.
        #[n(0)]
        event_source: WithdrawalEvent,
        #[n(1)]
        fail_reason: Option<String>,
    },
    #[n(13)]
    WithdrawalRedeemedEvent {
        /// The withdrawal gSol event.
        #[n(0)]
        event_source: WithdrawalEvent,
    },
}

#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct Event {
    /// The canister time at which the minter generated this event.
    #[n(0)]
    pub timestamp: u64,
    /// The event type.
    #[n(1)]
    pub payload: EventType,
}
