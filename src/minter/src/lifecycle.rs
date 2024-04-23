use crate::logs::INFO;
use crate::state::{
    audit::{process_event, replay_events, EventType},
    mutate_state, InvalidStateError, State, STATE,
};
use crate::storage::total_event_count;

use candid::{CandidType, Deserialize, Nat, Principal};
use minicbor::{Decode, Encode};
use num_bigint::ToBigUint;
use std::fmt::{Display, Formatter};

#[derive(CandidType, Deserialize, Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct InitArg {
    #[n(0)]
    pub solana_rpc_url: SolanaRpcUrl,
    #[n(1)]
    pub solana_contract_address: String,
    #[n(2)]
    pub solana_initial_signature: String,
    #[n(3)]
    pub ecdsa_key_name: String,
    #[cbor(n(4), with = "crate::cbor::principal")]
    pub ledger_id: Principal,
    #[cbor(n(5), with = "crate::cbor::nat")]
    pub minimum_withdrawal_amount: Nat,
}

impl TryFrom<InitArg> for State {
    type Error = InvalidStateError;
    fn try_from(
        InitArg {
            solana_rpc_url,
            solana_contract_address,
            solana_initial_signature,
            ecdsa_key_name,
            ledger_id,
            minimum_withdrawal_amount,
        }: InitArg,
    ) -> Result<Self, Self::Error> {
        let minimum_withdrawal_amount = minimum_withdrawal_amount.0.to_biguint().ok_or(
            InvalidStateError::InvalidMinimumWithdrawalAmount(
                "ERROR: minimum_withdrawal_amount is not a valid u256".to_string(),
            ),
        )?;

        let state = Self {
            solana_rpc_url,
            solana_contract_address,
            solana_initial_signature,
            ecdsa_key_name,
            ecdsa_public_key: None,
            ledger_id,
            minimum_withdrawal_amount,
            solana_last_known_signature: None,
            solana_signature_ranges: Default::default(),
            solana_signatures: Default::default(),
            invalid_events: Default::default(),
            accepted_events: Default::default(),
            minted_events: Default::default(),
            withdrawal_burned_events: Default::default(),
            withdrawal_redeemed_events: Default::default(),
            withdrawing_principals: Default::default(),
            burn_id_counter: 0,
            deposit_id_counter: 0,
            http_request_counter: 0,
            active_tasks: Default::default(),
        };

        state.validate_config()?;
        Ok(state)
    }
}

#[derive(CandidType, Deserialize, Clone, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct UpgradeArg {
    #[n(0)]
    pub solana_rpc_url: Option<SolanaRpcUrl>,
    #[n(1)]
    pub solana_contract_address: Option<String>,
    #[n(2)]
    pub solana_initial_signature: Option<String>,
    #[n(3)]
    pub ecdsa_key_name: Option<String>,
    #[cbor(n(4), with = "crate::cbor::nat::option")]
    pub minimum_withdrawal_amount: Option<Nat>,
}

pub fn post_upgrade(upgrade_args: Option<UpgradeArg>) {
    let start = ic_cdk::api::instruction_counter();

    STATE.with(|cell| {
        *cell.borrow_mut() = Some(replay_events());
    });
    if let Some(args) = upgrade_args {
        mutate_state(|s| process_event(s, EventType::Upgrade(args)))
    }

    let end = ic_cdk::api::instruction_counter();

    let event_count = total_event_count();
    let instructions_consumed = end - start;

    ic_canister_log::log!(
        INFO,
        "[upgrade]: replaying {event_count} events consumed {instructions_consumed} instructions ({} instructions per event on average)",
        instructions_consumed / event_count
    );
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum MinterArg {
    Init(InitArg),
    Upgrade(UpgradeArg),
}

#[derive(CandidType, Clone, Default, Deserialize, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct SolanaRpcUrl(#[n(1)] String);

impl SolanaRpcUrl {
    pub fn get(&self) -> &str {
        &self.0
    }
}

impl Display for SolanaRpcUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
