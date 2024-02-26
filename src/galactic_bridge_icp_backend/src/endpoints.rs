use crate::state::transactions::EthWithdrawalRequest;
use crate::tx::{SignedEip1559TransactionRequest, TransactionPrice};
use candid::{CandidType, Deserialize, Nat};
use icrc_ledger_types::icrc2::transfer_from::TransferFromError;
use minicbor::{Decode, Encode};
use std::fmt::{Display, Formatter};

#[derive(CandidType, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Eip1559TransactionPrice {
    pub gas_limit: Nat,
    pub max_fee_per_gas: Nat,
    pub max_priority_fee_per_gas: Nat,
    pub max_transaction_fee: Nat,
    pub timestamp: Option<u64>,
}

impl From<TransactionPrice> for Eip1559TransactionPrice {
    fn from(value: TransactionPrice) -> Self {
        Self {
            gas_limit: value.gas_limit.into(),
            max_fee_per_gas: value.max_fee_per_gas.into(),
            max_priority_fee_per_gas: value.max_priority_fee_per_gas.into(),
            max_transaction_fee: value.max_transaction_fee().into(),
            timestamp: None,
        }
    }
}

#[derive(CandidType, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct MinterInfo {
    pub minter_address: Option<String>,
    pub smart_contract_address: Option<String>,
    pub minimum_withdrawal_amount: Option<Nat>,
    pub ethereum_block_height: Option<CandidBlockTag>,
    pub last_observed_block_number: Option<Nat>,
    pub eth_balance: Option<Nat>,
    pub last_gas_fee_estimate: Option<GasFeeEstimate>,
}

#[derive(CandidType, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct GasFeeEstimate {
    pub max_fee_per_gas: Nat,
    pub max_priority_fee_per_gas: Nat,
    pub timestamp: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EthTransaction {
    pub transaction_hash: String,
}

impl From<&SignedEip1559TransactionRequest> for EthTransaction {
    fn from(value: &SignedEip1559TransactionRequest) -> Self {
        Self {
            transaction_hash: value.hash().to_string(),
        }
    }
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct RetrieveEthRequest {
    pub block_index: Nat,
}

#[derive(CandidType, Debug, Default, Deserialize, Clone, Encode, Decode, PartialEq, Eq)]
#[cbor(index_only)]
pub enum CandidBlockTag {
    /// The latest mined block.
    #[default]
    #[cbor(n(0))]
    Latest,
    /// The latest safe head block.
    /// See
    /// <https://www.alchemy.com/overviews/ethereum-commitment-levels#what-are-ethereum-commitment-levels>
    #[cbor(n(1))]
    Safe,
    /// The latest finalized block.
    /// See
    /// <https://www.alchemy.com/overviews/ethereum-commitment-levels#what-are-ethereum-commitment-levels>
    #[cbor(n(2))]
    Finalized,
}

impl From<EthWithdrawalRequest> for RetrieveEthRequest {
    fn from(value: EthWithdrawalRequest) -> Self {
        Self {
            block_index: candid::Nat::from(value.ledger_burn_index.get()),
        }
    }
}

#[derive(CandidType, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub enum RetrieveEthStatus {
    NotFound,
    Pending,
    TxCreated,
    TxSent(EthTransaction),
    TxFinalized(TxFinalizedStatus),
}

#[derive(CandidType, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub enum TxFinalizedStatus {
    Success(EthTransaction),
    PendingReimbursement(EthTransaction),
    Reimbursed {
        transaction_hash: String,
        reimbursed_amount: Nat,
        reimbursed_in_block: Nat,
    },
}

impl Display for RetrieveEthStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RetrieveEthStatus::NotFound => write!(f, "Not Found"),
            RetrieveEthStatus::Pending => write!(f, "Pending"),
            RetrieveEthStatus::TxCreated => write!(f, "Created"),
            RetrieveEthStatus::TxSent(tx) => write!(f, "Sent({})", tx.transaction_hash),
            RetrieveEthStatus::TxFinalized(tx_status) => match tx_status {
                TxFinalizedStatus::Success(tx) => write!(f, "Confirmed({})", tx.transaction_hash),
                TxFinalizedStatus::PendingReimbursement(tx) => {
                    write!(f, "PendingReimbursement({})", tx.transaction_hash)
                }
                TxFinalizedStatus::Reimbursed {
                    reimbursed_in_block,
                    transaction_hash,
                    reimbursed_amount,
                } => write!(
                    f,
                    "Failure({}, reimbursed: {} Wei in block: {})",
                    transaction_hash, reimbursed_amount, reimbursed_in_block
                ),
            },
        }
    }
}

#[derive(CandidType, Deserialize)]
pub struct WithdrawalArg {
    pub amount: Nat,
    pub recipient: String,
}

#[derive(CandidType, Deserialize, Debug, PartialEq)]
pub enum WithdrawalError {
    AmountTooLow { min_withdrawal_amount: Nat },
    InsufficientFunds { balance: Nat },
    InsufficientAllowance { allowance: Nat },
    RecipientAddressBlocked { address: String },
    TemporarilyUnavailable(String),
}

impl From<TransferFromError> for WithdrawalError {
    fn from(transfer_from_error: TransferFromError) -> Self {
        match transfer_from_error {
            TransferFromError::BadFee { expected_fee } => {
                panic!("bug: bad fee, expected fee: {expected_fee}")
            }
            TransferFromError::BadBurn { min_burn_amount } => {
                panic!("bug: bad burn, minimum burn amount: {min_burn_amount}")
            }
            TransferFromError::InsufficientFunds { balance } => Self::InsufficientFunds { balance },
            TransferFromError::InsufficientAllowance { allowance } => {
                Self::InsufficientAllowance { allowance }
            }
            TransferFromError::TooOld => panic!("bug: transfer too old"),
            TransferFromError::CreatedInFuture { ledger_time } => {
                panic!("bug: created in future, ledger time: {ledger_time}")
            }
            TransferFromError::Duplicate { duplicate_of } => {
                panic!("bug: duplicate transfer of: {duplicate_of}")
            }
            TransferFromError::TemporarilyUnavailable => Self::TemporarilyUnavailable(
                "ckETH ledger temporarily unavailable, try again".to_string(),
            ),
            TransferFromError::GenericError {
                error_code,
                message,
            } => Self::TemporarilyUnavailable(
                format!(
                    "ckETH ledger unreachable, error code: {error_code}, with message: {message}"
                )
                .to_string(),
            ),
        }
    }
}

pub mod events {
    use crate::lifecycle::init::InitArg;
    use crate::lifecycle::upgrade::UpgradeArg;
    use candid::{CandidType, Deserialize, Nat, Principal};
    use serde_bytes::ByteBuf;

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct GetEventsArg {
        pub start: u64,
        pub length: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct GetEventsResult {
        pub events: Vec<Event>,
        pub total_event_count: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Event {
        pub timestamp: u64,
        pub payload: EventPayload,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct EventSource {
        pub transaction_hash: String,
        pub log_index: Nat,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct AccessListItem {
        pub address: String,
        pub storage_keys: Vec<ByteBuf>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct UnsignedTransaction {
        // pub chain_id: Nat,
        pub nonce: Nat,
        pub max_priority_fee_per_gas: Nat,
        pub max_fee_per_gas: Nat,
        pub gas_limit: Nat,
        pub destination: String,
        pub value: Nat,
        pub data: ByteBuf,
        pub access_list: Vec<AccessListItem>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum TransactionStatus {
        Success,
        Failure,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct TransactionReceipt {
        pub block_hash: String,
        pub block_number: Nat,
        pub effective_gas_price: Nat,
        pub gas_used: Nat,
        pub status: TransactionStatus,
        pub transaction_hash: String,
    }

    #[derive(CandidType, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub enum EventPayload {
        Init(InitArg),
        Upgrade(UpgradeArg),
        AcceptedDeposit {
            transaction_hash: String,
            block_number: Nat,
            log_index: Nat,
            from_address: String,
            value: Nat,
            principal: Principal,
        },
        InvalidDeposit {
            event_source: EventSource,
            reason: String,
        },
        MintedCkEth {
            event_source: EventSource,
            mint_block_index: Nat,
        },
        SyncedToBlock {
            block_number: Nat,
        },
        AcceptedEthWithdrawalRequest {
            withdrawal_amount: Nat,
            destination: String,
            ledger_burn_index: Nat,
            from: Principal,
            from_subaccount: Option<[u8; 32]>,
            created_at: Option<u64>,
        },
        CreatedTransaction {
            withdrawal_id: Nat,
            transaction: UnsignedTransaction,
        },
        SignedTransaction {
            withdrawal_id: Nat,
            raw_transaction: String,
        },
        ReplacedTransaction {
            withdrawal_id: Nat,
            transaction: UnsignedTransaction,
        },
        FinalizedTransaction {
            withdrawal_id: Nat,
            transaction_receipt: TransactionReceipt,
        },
        ReimbursedEthWithdrawal {
            reimbursed_in_block: Nat,
            withdrawal_id: Nat,
            reimbursed_amount: Nat,
            transaction_hash: Option<String>,
        },
        SkippedBlock {
            block_number: Nat,
        },
    }
}
