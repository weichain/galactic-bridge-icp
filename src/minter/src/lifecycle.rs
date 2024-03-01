use crate::state::{InvalidStateError, State};
use candid::{CandidType, Deserialize, Nat, Principal};
use minicbor::{Decode, Encode};
use num_bigint::ToBigUint;
use std::fmt::{Display, Formatter};

#[derive(CandidType, Deserialize, Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct InitArg {
    #[n(0)]
    pub solana_network: SolanaNetwork,
    #[n(1)]
    pub solana_contract_address: String,
    #[n(2)]
    pub ecdsa_key_name: String,
    #[cbor(n(3), with = "crate::cbor::principal")]
    pub ledger_id: Principal,
    #[cbor(n(4), with = "crate::cbor::nat")]
    pub minimum_withdrawal_amount: Nat,
}

impl TryFrom<InitArg> for State {
    type Error = InvalidStateError;
    fn try_from(
        InitArg {
            solana_network,
            solana_contract_address,
            ecdsa_key_name,
            ledger_id,
            minimum_withdrawal_amount,
        }: InitArg,
    ) -> Result<Self, Self::Error> {
        // TODO: do conversion between types here

        let minimum_withdrawal_amount = minimum_withdrawal_amount.0.to_biguint().ok_or(
            InvalidStateError::InvalidMinimumWithdrawalAmount(
                "ERROR: minimum_withdrawal_amount is not a valid u256".to_string(),
            ),
        )?;

        let state = Self {
            solana_network,
            solana_contract_address,
            ecdsa_key_name,
            ecdsa_public_key: None,
            ledger_id,
            minimum_withdrawal_amount,
        };
        state.validate_config()?;
        Ok(state)
    }
}

#[derive(CandidType, Deserialize, Clone, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct UpgradeArg {
    #[n(0)]
    pub solana_contract_address: Option<String>,
    #[cbor(n(1), with = "crate::cbor::nat::option")]
    pub minimum_withdrawal_amount: Option<Nat>,
}

// TODO: implement it
// pub fn post_upgrade(upgrade_args: Option<UpgradeArg>) {}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum MinterArg {
    InitArg(InitArg),
    UpgradeArg(UpgradeArg),
}

#[derive(
    CandidType, Clone, Copy, Default, Deserialize, Debug, Eq, PartialEq, Hash, Encode, Decode,
)]
#[cbor(index_only)]
pub enum SolanaNetwork {
    #[n(1)]
    Mainnet,
    #[n(2)]
    #[default]
    Testnet,
}

impl Display for SolanaNetwork {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SolanaNetwork::Mainnet => write!(f, "Solana Mainnet"),
            SolanaNetwork::Testnet => write!(f, "Solana Testnet"),
        }
    }
}
