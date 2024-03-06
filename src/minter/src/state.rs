use crate::constants::DERIVATION_PATH;
use crate::lifecycle::SolanaNetwork;
use crate::logs::DEBUG;

use candid::Principal;
use ic_canister_log::log;
use ic_cdk::api::management_canister::ecdsa::EcdsaPublicKeyResponse;
use num_bigint::BigUint;
use std::{cell::RefCell, collections::HashSet};
use strum_macros::EnumIter;

thread_local! {
  pub static STATE: RefCell<Option<State>> = RefCell::default();
}

#[derive(Debug, Eq, PartialEq)]
pub enum InvalidStateError {
    InvalidEcdsaKeyName(String),
    InvalidLedgerId(String),
    InvalidSolanaContractAddress(String),
    InvalidMinimumWithdrawalAmount(String),
    InvalidInitialTransaction(String),
}

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, EnumIter)]
pub enum TaskType {
    MintCkSol,
    RetrieveSol,
    ScrapSolLogs,
    // TODO: what is Reimbursement and RetrieveSol
    Reimbursement,
}

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    // solana config
    pub solana_network: SolanaNetwork,
    pub solana_contract_address: String,
    pub solana_initial_transaction: String,

    // icp config
    pub ecdsa_key_name: String,
    // raw format of the public key
    pub ecdsa_public_key: Option<EcdsaPublicKeyResponse>,
    pub ledger_id: Principal,
    pub minimum_withdrawal_amount: BigUint,

    // internals
    pub last_scraped_transaction: Option<String>,

    /// Locks preventing concurrent execution timer tasks
    pub active_tasks: HashSet<TaskType>,
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
                "solana_contract_address cannot be empty".to_string(),
            ));
        }
        if self.solana_initial_transaction.trim().is_empty() {
            return Err(InvalidStateError::InvalidInitialTransaction(
                "solana_initial_transaction cannot be empty".to_string(),
            ));
        }
        if self.minimum_withdrawal_amount == BigUint::from(0u8) {
            return Err(InvalidStateError::InvalidMinimumWithdrawalAmount(
                "minimum_withdrawal_amount must be positive".to_string(),
            ));
        }
        Ok(())
    }

    // compressed public key in hex format - 33 bytes
    pub fn compressed_public_key(&self) -> String {
        let public_key = match &self.ecdsa_public_key {
            Some(response) => &response.public_key,
            None => ic_cdk::trap("BUG: public key is not initialized"),
        };

        hex::encode(&public_key)
    }

    // uncompressed public key in hex format - 65 bytes
    pub fn uncompressed_public_key(&self) -> String {
        use libsecp256k1::{PublicKey, PublicKeyFormat};

        let public_key = match &self.ecdsa_public_key {
            Some(response) => &response.public_key,
            None => ic_cdk::trap("BUG: public key is not initialized"),
        };

        let uncompressed_pubkey =
            PublicKey::parse_slice(&public_key, Some(PublicKeyFormat::Compressed))
                .expect("failed to deserialize sec1 encoding into public key")
                .serialize();

        hex::encode(uncompressed_pubkey)
    }

    pub const fn solana_network(&self) -> SolanaNetwork {
        self.solana_network
    }

    pub fn get_last_scraped_transaction(&self) -> String {
        if let Some(tx) = &self.last_scraped_transaction {
            tx.to_string()
        } else {
            self.solana_initial_transaction.to_string()
        }
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

pub async fn lazy_call_ecdsa_public_key() -> ic_crypto_ecdsa_secp256k1::PublicKey {
    use ic_cdk::api::management_canister::ecdsa::{
        ecdsa_public_key, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    };

    fn to_public_key(response: &EcdsaPublicKeyResponse) -> ic_crypto_ecdsa_secp256k1::PublicKey {
        ic_crypto_ecdsa_secp256k1::PublicKey::deserialize_sec1(&response.public_key).unwrap_or_else(
            |e| ic_cdk::trap(&format!("failed to decode minter's public key: {:?}", e)),
        )
    }

    if let Some(ecdsa_pk_response) = read_state(|s| s.ecdsa_public_key.clone()) {
        return to_public_key(&ecdsa_pk_response);
    }

    let key_name = read_state(|s| s.ecdsa_key_name.clone());

    log!(DEBUG, "Fetching the ECDSA public key {key_name}");

    let (response,) = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: DERIVATION_PATH.into_iter().map(|x| x.to_vec()).collect(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: key_name,
        },
    })
    .await
    .unwrap_or_else(|(error_code, message)| {
        ic_cdk::trap(&format!(
            "failed to get minter's public key: {} (error code = {:?})",
            message, error_code,
        ))
    });

    mutate_state(|s| s.ecdsa_public_key = Some(response.clone()));

    to_public_key(&response)
}
