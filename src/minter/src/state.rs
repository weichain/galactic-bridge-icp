use crate::lifecycle::SolanaNetwork;
use candid::Principal;
use num_bigint::BigUint;
use std::cell::RefCell;

use ic_cdk::api::management_canister::ecdsa::EcdsaPublicKeyResponse;

thread_local! {
  pub static STATE: RefCell<Option<State>> = RefCell::default();
}

#[derive(Debug, Eq, PartialEq)]
pub enum InvalidStateError {
    InvalidEcdsaKeyName(String),
    InvalidLedgerId(String),
    InvalidSolanaContractAddress(String),
    InvalidMinimumWithdrawalAmount(String),
    // TODO: implement errors
    // InvalidLastScrapedTransaction(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    pub solana_network: SolanaNetwork,
    pub solana_contract_address: String,

    pub ecdsa_key_name: String,
    pub ecdsa_public_key: Option<EcdsaPublicKeyResponse>,
    pub ledger_id: Principal,
    pub minimum_withdrawal_amount: BigUint,
    // TODO: implement types
    // pub first_scraped_transaction: SomeType,
    // pub last_scraped_transaction: SomeType,
    // pub last_observed_transaction: Option<SomeType>,

    // TODO: implement type
    // pub solana_transactions: SomeSolanaTransactionsType,

    // TODO: implement types
    // pub events_to_mint: BTreeMap<EventSource, ReceivedEthEvent>,
    // pub minted_events: BTreeMap<EventSource, MintedEvent>,
    // pub invalid_events: BTreeMap<EventSource, String>,

    // TODO: no clue if I need this
    // /// Current balance of ETH held by minter.
    // /// Computed based on audit events.
    // pub eth_balance: EthBalance,
    // /// Per-principal lock for pending_retrieve_eth_requests
    // pub retrieve_eth_principals: BTreeSet<Principal>,
    // /// Locks preventing concurrent execution timer tasks
    // pub active_tasks: HashSet<TaskType>,
    // /// Number of HTTP outcalls since the last upgrade.
    // /// Used to correlate request and response in logs.
    // pub http_request_counter: u64,
    // /// Number of HTTP outcalls since the last upgrade.
    // /// Used to correlate request and response in logs.
    // pub http_request_counter: u64,
}

impl State {
    pub fn validate_config(&self) -> Result<(), InvalidStateError> {
        if self.ecdsa_key_name.trim().is_empty() {
            return Err(InvalidStateError::InvalidEcdsaKeyName(
                "ecdsa_key_name cannot be blank".to_string(),
            ));
        }
        if self.ledger_id == Principal::anonymous() {
            return Err(InvalidStateError::InvalidLedgerId(
                "ledger_id cannot be the anonymous principal".to_string(),
            ));
        }
        if self.solana_contract_address.trim().is_empty() {
            return Err(InvalidStateError::InvalidSolanaContractAddress(
                "solana_contract_address cannot be the zero address".to_string(),
            ));
        }
        if self.minimum_withdrawal_amount == BigUint::from(0u8) {
            return Err(InvalidStateError::InvalidMinimumWithdrawalAmount(
                "minimum_withdrawal_amount must be positive".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn read_state<R>(f: impl FnOnce(&State) -> R) -> R {
    STATE.with(|s| f(s.borrow().as_ref().expect("BUG: state is not initialized")))
}

/// Mutates (part of) the current state using `f`.
///
/// Panics if there is no state.
pub fn mutate_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut State) -> R,
{
    STATE.with(|s| {
        f(s.borrow_mut()
            .as_mut()
            .expect("BUG: state is not initialized"))
    })
}
