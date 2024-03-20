use crate::constants::DERIVATION_PATH;
use crate::events::{
    ReceivedSolEvent, Retriable, SolanaSignature, SolanaSignatureRange, WithdrawalEvent,
};
use crate::lifecycle::{SolanaNetwork, UpgradeArg};
use crate::logs::DEBUG;

use candid::Principal;
use ic_canister_log::log;
use ic_cdk::api::management_canister::ecdsa::EcdsaPublicKeyResponse;
use num_bigint::BigUint;
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap, HashSet},
};
use strum_macros::EnumIter;

pub mod audit;
pub mod event;

thread_local! {
  pub static STATE: RefCell<Option<State>> = RefCell::default();
}

#[derive(Debug, Eq, PartialEq)]
pub enum InvalidStateError {
    InvalidEcdsaKeyName(String),
    InvalidLedgerId(String),
    InvalidSolanaContractAddress(String),
    InvalidMinimumWithdrawalAmount(String),
    InvalidSolanaInitialSignature(String),
}

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, EnumIter)]
pub enum TaskType {
    GetLatestSignature,
    ScrapSignatureRanges,
    ScrapSignatures,
    MintCkSol,
}

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    // solana config
    pub solana_network: SolanaNetwork,
    pub solana_contract_address: String,
    pub solana_initial_signature: String,

    // icp config
    pub ecdsa_key_name: String,
    // raw format of the public key
    pub ecdsa_public_key: Option<EcdsaPublicKeyResponse>,
    pub ledger_id: Principal,
    pub minimum_withdrawal_amount: BigUint,

    // scrapper config
    pub solana_last_known_signature: Option<String>,

    pub solana_signature_ranges: HashMap<String, SolanaSignatureRange>,
    pub solana_signatures: HashMap<String, SolanaSignature>,

    // invalid transactions - cannot be parsed, does not hold deposit event, blocked user, etc.
    pub invalid_events: HashMap<String, SolanaSignature>,
    // valid transaction events
    pub accepted_events: HashMap<String, ReceivedSolEvent>,
    // minted events
    pub minted_events: HashMap<String, ReceivedSolEvent>,
    // withdrawal events
    pub withdrawal_events: HashMap<u64, WithdrawalEvent>,

    // Withdrawal requests that are currently being processed
    pub withdrawing_principals: BTreeSet<Principal>,
    // Unique identifier for each withdrawal
    pub withdrawal_id_counter: u64,

    /// Number of HTTP outcalls since the last upgrade.
    /// Used to correlate request and response in logs.
    pub http_request_counter: u64,

    /// Locks preventing concurrent execution timer tasks
    pub active_tasks: HashSet<TaskType>,
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
        if self.solana_initial_signature.trim().is_empty() {
            return Err(InvalidStateError::InvalidSolanaInitialSignature(
                "solana_initial_signature cannot be empty".to_string(),
            ));
        }
        if self.minimum_withdrawal_amount == BigUint::from(0u8) {
            return Err(InvalidStateError::InvalidMinimumWithdrawalAmount(
                "minimum_withdrawal_amount must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn upgrade(&mut self, upgrade_args: UpgradeArg) -> () {}

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

    pub fn record_solana_last_known_signature(&mut self, sig: &String) {
        self.solana_last_known_signature = Some(sig.to_string());
    }

    pub fn get_solana_last_known_signature(&self) -> String {
        match &self.solana_last_known_signature {
            Some(sig) => sig.to_string(),
            None => self.solana_initial_signature.to_string(),
        }
    }

    pub fn record_solana_signature_range(&mut self, range: SolanaSignatureRange) {
        let key = range_key(&range.before_sol_sig, &range.until_sol_sig);

        match self.solana_signature_ranges.contains_key(&key) {
            true => {
                panic!("Attempted to record existing range: {key} .");
            }
            false => {
                self.solana_signature_ranges.insert(key, range);
            }
        }
    }

    pub fn retry_solana_signature_range(
        &mut self,
        old_range: SolanaSignatureRange,
        new_range: Option<SolanaSignatureRange>,
    ) {
        let old_key = range_key(&old_range.before_sol_sig, &old_range.until_sol_sig);

        match self.solana_signature_ranges.remove(&old_key) {
            Some(mut old_range) => {
                match new_range {
                    // if it is a sub range of previously failed range failed, remove the old range and add the new range
                    Some(new_range) => {
                        self.record_solana_signature_range(new_range);
                    }
                    None => {
                        // in case range exists, increment the retries
                        old_range.increment_retries();
                        self.solana_signature_ranges
                            .insert(old_key.to_string(), old_range);
                    }
                }
            }
            None => panic!("Attempted to re-record NON existing range: {old_key} ."),
        }
    }

    pub fn remove_solana_signature_range(&mut self, range: &SolanaSignatureRange) {
        let key = range_key(&range.before_sol_sig, &range.until_sol_sig);

        match self.solana_signature_ranges.remove(&key) {
            Some(_) => {}
            None => panic!("Attempted to remove NON existing range: {key} ."),
        };
    }

    pub fn record_solana_signature(&mut self, sig: SolanaSignature) {
        match self.solana_signatures.contains_key(&sig.sol_sig) {
            true => {
                // if it exists - increment the retries
                let mut existing_signature = self.solana_signatures.remove(&sig.sol_sig).unwrap();

                existing_signature.increment_retries();
                self.solana_signatures
                    .insert(sig.sol_sig.to_string(), existing_signature);
            }
            false => {
                // if it does not exist - add it
                self.solana_signatures.insert(sig.sol_sig.to_string(), sig);
            }
        }
    }

    pub fn record_invalid_event(&mut self, sig: SolanaSignature) {
        let key = &sig.sol_sig;

        match self.solana_signatures.remove(key) {
            Some(event) => event,
            None => panic!("Attempted to remove NON existing solana signature {key} ."),
        };

        assert!(
            self.invalid_events.contains_key(key),
            "Attempted to record existing invalid event: {key} ."
        );

        self.invalid_events.insert(key.to_string(), sig);
    }

    pub fn record_accepted_event(&mut self, deposit: ReceivedSolEvent) {
        let key = &deposit.sol_sig;

        match self.solana_signatures.remove(key) {
            Some(event) => event,
            None => panic!("Attempted to remove NON existing solana signature {key} ."),
        };

        match self.accepted_events.contains_key(key) {
            true => {
                let mut existing_event = self.accepted_events.remove(key).unwrap();
                existing_event.increment_retries();
                self.accepted_events.insert(key.to_string(), existing_event)
            }
            false => self.accepted_events.insert(key.to_string(), deposit),
        };
    }

    pub fn record_minted_event(&mut self, deposit: ReceivedSolEvent) {
        let key = &deposit.sol_sig;

        _ = match self.accepted_events.remove(key) {
            Some(event) => event,
            None => panic!("Attempted to remove NON existing accepted event: {key} ."),
        };

        assert!(
            self.minted_events.contains_key(key),
            "Attempted to record existing minted event: {key}."
        );

        _ = self.minted_events.insert(key.to_string(), deposit);
    }

    pub fn record_withdrawal_event(&mut self, withdrawal: WithdrawalEvent) {
        let key = withdrawal.id;
        assert!(
            self.withdrawal_events.contains_key(&key),
            "Attempted to record existing withdrawal event: {key}."
        );

        _ = self.withdrawal_events.insert(key, withdrawal);
    }

    pub fn next_request_id(&mut self) -> u64 {
        let current_request_id = self.http_request_counter;
        // overflow is not an issue here because we only use `next_request_id` to correlate
        // requests and responses in logs.
        self.http_request_counter = self.http_request_counter.wrapping_add(1);
        current_request_id
    }

    pub fn next_withdrawal_id(&mut self) -> u64 {
        let current_withdrawal_id = self.withdrawal_id_counter;
        self.withdrawal_id_counter = self.withdrawal_id_counter.wrapping_add(1);
        current_withdrawal_id
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

fn range_key(start: &String, end: &String) -> String {
    return format!("{}-{}", start, end);
}
