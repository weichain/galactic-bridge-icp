use crate::address::ecdsa_public_key_to_address;
use crate::eth_logs::{EventSource, ReceivedEthEvent};
use crate::eth_rpc::BlockTag;
use crate::lifecycle::upgrade::UpgradeArg;
use crate::lifecycle::SolanaNetwork;
use crate::logs::DEBUG;
use crate::numeric::{BlockNumber, LedgerBurnIndex, LedgerMintIndex, TransactionNonce, Wei};
use crate::solana_rpc_client::responses::{TransactionReceipt, TransactionStatus};
use crate::tx::TransactionPriceEstimate;
use candid::Principal;
use ic_canister_log::log;
use ic_cdk::api::management_canister::ecdsa::EcdsaPublicKeyResponse;
use ic_crypto_ecdsa_secp256k1::PublicKey;
use ic_ethereum_types::Address;
use std::cell::RefCell;
use std::collections::{btree_map, BTreeMap, BTreeSet, HashSet};
use strum_macros::EnumIter;
use transactions::EthTransactions;

pub mod audit;
pub mod event;
pub mod transactions;

thread_local! {
    pub static STATE: RefCell<Option<State>> = RefCell::default();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintedEvent {
    pub deposit_event: ReceivedEthEvent,
    pub mint_block_index: LedgerMintIndex,
}

impl MintedEvent {
    pub fn source(&self) -> EventSource {
        self.deposit_event.source()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct State {
    pub solana_network: SolanaNetwork,
    pub ecdsa_key_name: String,
    pub ledger_id: Principal,
    pub ethereum_contract_address: Option<Address>,
    pub ecdsa_public_key: Option<EcdsaPublicKeyResponse>,
    pub minimum_withdrawal_amount: Wei,
    pub ethereum_block_height: BlockTag,
    pub first_scraped_block_number: BlockNumber,
    pub last_scraped_block_number: BlockNumber,
    pub last_observed_block_number: Option<BlockNumber>,
    pub events_to_mint: BTreeMap<EventSource, ReceivedEthEvent>,
    pub minted_events: BTreeMap<EventSource, MintedEvent>,
    pub invalid_events: BTreeMap<EventSource, String>,
    pub eth_transactions: EthTransactions,
    pub skipped_blocks: BTreeSet<BlockNumber>,

    /// Current balance of ETH held by minter.
    /// Computed based on audit events.
    pub eth_balance: EthBalance,
    /// Per-principal lock for pending_retrieve_eth_requests
    pub retrieve_eth_principals: BTreeSet<Principal>,

    /// Locks preventing concurrent execution timer tasks
    pub active_tasks: HashSet<TaskType>,

    /// Number of HTTP outcalls since the last upgrade.
    /// Used to correlate request and response in logs.
    pub http_request_counter: u64,

    pub last_transaction_price_estimate: Option<(u64, TransactionPriceEstimate)>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum InvalidStateError {
    InvalidTransactionNonce(String),
    InvalidEcdsaKeyName(String),
    InvalidLedgerId(String),
    InvalidEthereumContractAddress(String),
    InvalidMinimumWithdrawalAmount(String),
    InvalidLastScrapedBlockNumber(String),
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
        if self
            .ethereum_contract_address
            .iter()
            .any(|address| address == &Address::ZERO)
        {
            return Err(InvalidStateError::InvalidEthereumContractAddress(
                "ethereum_contract_address cannot be the zero address".to_string(),
            ));
        }
        if self.minimum_withdrawal_amount == Wei::ZERO {
            return Err(InvalidStateError::InvalidMinimumWithdrawalAmount(
                "minimum_withdrawal_amount must be positive".to_string(),
            ));
        }
        Ok(())
    }

    pub fn minter_address(&self) -> Option<Address> {
        let pubkey = PublicKey::deserialize_sec1(&self.ecdsa_public_key.as_ref()?.public_key)
            .unwrap_or_else(|e| {
                ic_cdk::trap(&format!("failed to decode minter's public key: {:?}", e))
            });
        Some(ecdsa_public_key_to_address(&pubkey))
    }

    fn record_event_to_mint(&mut self, event: &ReceivedEthEvent) {
        let event_source = event.source();
        assert!(
            !self.events_to_mint.contains_key(&event_source),
            "there must be no two different events with the same source"
        );
        assert!(!self.minted_events.contains_key(&event_source));
        assert!(!self.invalid_events.contains_key(&event_source));

        self.events_to_mint.insert(event_source, event.clone());

        self.update_eth_balance_upon_deposit(event)
    }

    pub fn has_events_to_mint(&self) -> bool {
        !self.events_to_mint.is_empty()
    }

    fn record_invalid_deposit(&mut self, source: EventSource, error: String) -> bool {
        assert!(
            !self.events_to_mint.contains_key(&source),
            "attempted to mark an accepted event as invalid"
        );
        assert!(
            !self.minted_events.contains_key(&source),
            "attempted to mark a minted event {source:?} as invalid"
        );

        match self.invalid_events.entry(source) {
            btree_map::Entry::Occupied(_) => false,
            btree_map::Entry::Vacant(entry) => {
                entry.insert(error);
                true
            }
        }
    }

    fn record_successful_mint(&mut self, source: EventSource, mint_block_index: LedgerMintIndex) {
        assert!(
            !self.invalid_events.contains_key(&source),
            "attempted to mint an event previously marked as invalid {source:?}"
        );
        let deposit_event = match self.events_to_mint.remove(&source) {
            Some(event) => event,
            None => panic!("attempted to mint ckETH for an unknown event {source:?}"),
        };

        assert_eq!(
            self.minted_events.insert(
                source,
                MintedEvent {
                    deposit_event,
                    mint_block_index
                }
            ),
            None,
            "attempted to mint ckETH twice for the same event {source:?}"
        );
    }

    pub fn record_finalized_transaction(
        &mut self,
        withdrawal_id: &LedgerBurnIndex,
        receipt: &TransactionReceipt,
    ) {
        self.eth_transactions
            .record_finalized_transaction(*withdrawal_id, receipt.clone());
        self.update_eth_balance_upon_withdrawal(withdrawal_id, receipt);
    }

    pub fn next_request_id(&mut self) -> u64 {
        let current_request_id = self.http_request_counter;
        // overflow is not an issue here because we only use `next_request_id` to correlate
        // requests and responses in logs.
        self.http_request_counter = self.http_request_counter.wrapping_add(1);
        current_request_id
    }

    fn update_eth_balance_upon_deposit(&mut self, event: &ReceivedEthEvent) {
        self.eth_balance.eth_balance_add(event.value);
    }

    fn update_eth_balance_upon_withdrawal(
        &mut self,
        withdrawal_id: &LedgerBurnIndex,
        receipt: &TransactionReceipt,
    ) {
        let tx_fee = receipt.effective_transaction_fee();
        let tx = self
            .eth_transactions
            .finalized_tx
            .get_alt(withdrawal_id)
            .expect("BUG: missing finalized transaction");
        let charged_tx_fee = tx.transaction_price().max_transaction_fee();
        let unspent_tx_fee = charged_tx_fee.checked_sub(tx_fee).expect(
            "BUG: charged transaction fee MUST always be at least the effective transaction fee",
        );
        let debited_amount = match receipt.status {
            TransactionStatus::Success => tx
                .transaction()
                .amount
                .checked_add(tx_fee)
                .expect("BUG: debited amount always fits into U256"),
            TransactionStatus::Failure => tx_fee,
        };
        self.eth_balance.eth_balance_sub(debited_amount);
        self.eth_balance.total_effective_tx_fees_add(tx_fee);
        self.eth_balance.total_unspent_tx_fees_add(unspent_tx_fee);
    }

    pub fn record_skipped_block(&mut self, block_number: BlockNumber) {
        assert!(
            self.skipped_blocks.insert(block_number),
            "BUG: block {} was already skipped",
            block_number
        );
    }

    pub const fn solana_network(&self) -> SolanaNetwork {
        self.solana_network
    }

    pub const fn ethereum_block_height(&self) -> BlockTag {
        self.ethereum_block_height
    }

    fn upgrade(&mut self, upgrade_args: UpgradeArg) -> Result<(), InvalidStateError> {
        use std::str::FromStr;

        let UpgradeArg {
            next_transaction_nonce,
            minimum_withdrawal_amount,
            ethereum_contract_address,
            ethereum_block_height,
        } = upgrade_args;
        if let Some(nonce) = next_transaction_nonce {
            let nonce = TransactionNonce::try_from(nonce)
                .map_err(|e| InvalidStateError::InvalidTransactionNonce(format!("ERROR: {}", e)))?;
            self.eth_transactions.update_next_transaction_nonce(nonce);
        }
        if let Some(amount) = minimum_withdrawal_amount {
            let minimum_withdrawal_amount = Wei::try_from(amount).map_err(|e| {
                InvalidStateError::InvalidMinimumWithdrawalAmount(format!("ERROR: {}", e))
            })?;
            self.minimum_withdrawal_amount = minimum_withdrawal_amount;
        }
        if let Some(address) = ethereum_contract_address {
            let ethereum_contract_address = Address::from_str(&address).map_err(|e| {
                InvalidStateError::InvalidEthereumContractAddress(format!("ERROR: {}", e))
            })?;
            self.ethereum_contract_address = Some(ethereum_contract_address);
        }
        if let Some(block_height) = ethereum_block_height {
            self.ethereum_block_height = block_height.into();
        }
        self.validate_config()
    }

    /// Checks whether two states are equivalent.
    pub fn is_equivalent_to(&self, other: &Self) -> Result<(), String> {
        // We define the equivalence using the upgrade procedure.
        // Replaying the event log won't produce exactly the same state we had before the upgrade,
        // but a state that equivalent for all practical purposes.
        //
        // For example, we don't compare:
        // 1. Computed fields and caches, such as `ecdsa_public_key`.
        // 2. Transient fields, such as `active_tasks`.
        use ic_utils_ensure::ensure_eq;

        ensure_eq!(self.solana_network, other.solana_network);
        ensure_eq!(self.ledger_id, other.ledger_id);
        ensure_eq!(self.ecdsa_key_name, other.ecdsa_key_name);
        ensure_eq!(
            self.ethereum_contract_address,
            other.ethereum_contract_address
        );
        ensure_eq!(
            self.minimum_withdrawal_amount,
            other.minimum_withdrawal_amount
        );
        ensure_eq!(
            self.first_scraped_block_number,
            other.first_scraped_block_number
        );
        ensure_eq!(
            self.last_scraped_block_number,
            other.last_scraped_block_number
        );
        ensure_eq!(self.ethereum_block_height, other.ethereum_block_height);
        ensure_eq!(self.events_to_mint, other.events_to_mint);
        ensure_eq!(self.minted_events, other.minted_events);
        ensure_eq!(self.invalid_events, other.invalid_events);

        self.eth_transactions
            .is_equivalent_to(&other.eth_transactions)
    }

    pub fn eth_balance(&self) -> &EthBalance {
        &self.eth_balance
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

pub async fn lazy_call_ecdsa_public_key() -> PublicKey {
    use ic_cdk::api::management_canister::ecdsa::{
        ecdsa_public_key, EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyArgument,
    };

    fn to_public_key(response: &EcdsaPublicKeyResponse) -> PublicKey {
        PublicKey::deserialize_sec1(&response.public_key).unwrap_or_else(|e| {
            ic_cdk::trap(&format!("failed to decode minter's public key: {:?}", e))
        })
    }

    if let Some(ecdsa_pk_response) = read_state(|s| s.ecdsa_public_key.clone()) {
        return to_public_key(&ecdsa_pk_response);
    }
    let key_name = read_state(|s| s.ecdsa_key_name.clone());
    log!(DEBUG, "Fetching the ECDSA public key {key_name}");
    let (response,) = ecdsa_public_key(EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: crate::MAIN_DERIVATION_PATH
            .into_iter()
            .map(|x| x.to_vec())
            .collect(),
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

pub async fn minter_address() -> Address {
    ecdsa_public_key_to_address(&lazy_call_ecdsa_public_key().await)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthBalance {
    /// Amount of ETH controlled by the minter's address via tECDSA.
    /// Note that invalid deposits are not accounted for and so so this value
    /// might be less than what is displayed by Etherscan
    /// or retrieved by the JSON-RPC call `eth_getBalance`.
    /// Also some transactions may have gone directly to the minter's address
    /// without going via the helper smart contract.
    eth_balance: Wei,
    /// Total amount of fees across all finalized transactions ckETH -> ETH.
    total_effective_tx_fees: Wei,
    /// Total amount of fees that were charged to the user during the withdrawal
    /// but not consumed by the finalized transaction ckETH -> ETH
    total_unspent_tx_fees: Wei,
}

impl Default for EthBalance {
    fn default() -> Self {
        Self {
            eth_balance: Wei::ZERO,
            total_effective_tx_fees: Wei::ZERO,
            total_unspent_tx_fees: Wei::ZERO,
        }
    }
}

impl EthBalance {
    fn eth_balance_add(&mut self, value: Wei) {
        self.eth_balance = self.eth_balance.checked_add(value).unwrap_or_else(|| {
            panic!(
                "BUG: overflow when adding {} to {}",
                value, self.eth_balance
            )
        })
    }

    fn eth_balance_sub(&mut self, value: Wei) {
        self.eth_balance = self.eth_balance.checked_sub(value).unwrap_or_else(|| {
            panic!(
                "BUG: underflow when subtracting {} from {}",
                value, self.eth_balance
            )
        })
    }

    fn total_effective_tx_fees_add(&mut self, value: Wei) {
        self.total_effective_tx_fees = self
            .total_effective_tx_fees
            .checked_add(value)
            .unwrap_or_else(|| {
                panic!(
                    "BUG: overflow when adding {} to {}",
                    value, self.total_effective_tx_fees
                )
            })
    }

    fn total_unspent_tx_fees_add(&mut self, value: Wei) {
        self.total_unspent_tx_fees = self
            .total_unspent_tx_fees
            .checked_add(value)
            .unwrap_or_else(|| {
                panic!(
                    "BUG: overflow when adding {} to {}",
                    value, self.total_unspent_tx_fees
                )
            })
    }

    pub fn eth_balance(&self) -> Wei {
        self.eth_balance
    }
    pub fn total_effective_tx_fees(&self) -> Wei {
        self.total_effective_tx_fees
    }

    pub fn total_unspent_tx_fees(&self) -> Wei {
        self.total_unspent_tx_fees
    }
}

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, EnumIter)]
pub enum TaskType {
    MintCkEth,
    RetrieveEth,
    ScrapEthLogs,
    Reimbursement,
}
