use crate::constants::DERIVATION_PATH;
use crate::withdraw::Coupon;

use candid::Principal;
use ic_cdk::api::{
    call::RejectionCode,
    management_canister::ecdsa::{
        sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, SignWithEcdsaArgument, SignWithEcdsaResponse,
    },
};
use ic_stable_structures::Storable;
use icrc_ledger_types::icrc1::transfer::Memo;
use minicbor::{Decode, Encode};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub trait Retriable {
    fn get_retries(&self) -> u8;
    fn increment_retries(&mut self);
    fn reset_retries(&mut self);
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SolanaSignatureRange {
    #[n(0)]
    pub before_sol_sig: String,
    #[n(1)]
    pub until_sol_sig: String,
    #[n(2)]
    retries: u8,
}

impl SolanaSignatureRange {
    // Constructor function to create a new SolanaSignatureRange
    pub fn new(before: String, until: String) -> Self {
        SolanaSignatureRange {
            before_sol_sig: before,
            until_sol_sig: until,
            retries: 0,
        }
    }
}

impl Retriable for SolanaSignatureRange {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }

    fn reset_retries(&mut self) {
        self.retries = 0;
    }
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SolanaSignature {
    #[n(0)]
    pub sol_sig: String,
    #[n(1)]
    retries: u8,
}

impl SolanaSignature {
    // Constructor function to create a new SolanaSignature
    pub fn new(signature: String) -> Self {
        SolanaSignature {
            sol_sig: signature,
            retries: 0,
        }
    }
}

impl Retriable for SolanaSignature {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }

    fn reset_retries(&mut self) {
        self.retries = 0;
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct ReceivedSolEvent {
    #[n(0)]
    pub from_sol_address: String,
    #[cbor(n(1), with = "crate::cbor::principal")]
    pub to_icp_address: Principal,
    #[n(2)]
    pub amount: u64,
    #[n(3)]
    pub sol_sig: String,
    #[n(4)]
    icp_mint_block_index: Option<u64>,
    #[n(5)]
    retries: u8,
}

impl ReceivedSolEvent {
    pub fn update_mint_block_index(&mut self, block_index: u64) {
        self.icp_mint_block_index = Some(block_index);
    }

    pub fn get_mint_block_index(&self) -> Option<u64> {
        self.icp_mint_block_index
    }
}

impl Retriable for ReceivedSolEvent {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }

    fn reset_retries(&mut self) {
        self.retries = 0;
    }
}

impl From<ReceivedSolEvent> for Memo {
    fn from(event: ReceivedSolEvent) -> Self {
        let bytes = serde_cbor::ser::to_vec(&event).expect("Failed to serialize ReceivedSolEvent");
        Memo::from(bytes)
    }
}

impl From<(&str, &str, &str)> for ReceivedSolEvent {
    fn from(data: (&str, &str, &str)) -> Self {
        use base64::prelude::*;
        let (sol_sig, from_address, encode_data) = data;

        let bytes = BASE64_STANDARD.decode(encode_data).unwrap();

        let amount_bytes = &bytes[bytes.len() - 8..];
        // TODO: maybe convert to BigUint
        let mut value: u64 = 0;
        for i in 0..8 {
            value |= (amount_bytes[i] as u64) << (i * 8);
        }

        let address_bytes = &bytes[12..bytes.len() - 8];
        // String::from_utf8_lossy(&address_bytes);
        let principal = Principal::from_bytes(std::borrow::Cow::Borrowed(address_bytes));

        ReceivedSolEvent {
            from_sol_address: from_address.to_string(),
            to_icp_address: principal,
            amount: value,
            sol_sig: sol_sig.to_string(),
            icp_mint_block_index: None,
            retries: 0,
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct WithdrawalEvent {
    #[n(0)]
    pub id: u64,
    #[cbor(n(1), with = "crate::cbor::principal")]
    pub from_icp_address: Principal,
    #[n(2)]
    pub to_sol_address: String,
    #[n(3)]
    pub amount: u64,
    #[n(4)]
    pub timestamp: u64,
    #[n(5)]
    pub icp_burn_block_index: u64,
}

impl WithdrawalEvent {
    pub async fn to_coupon(&self) -> Coupon {
        let (serialized_coupon, signature_hex) = self.sign_with_ecdsa().await;
        let icp_public_key_hex = crate::state::read_state(|s| s.uncompressed_public_key());

        let mut response = Coupon::new(serialized_coupon, signature_hex, icp_public_key_hex);
        response.y_parity();

        response
    }

    async fn sign_with_ecdsa(&self) -> (String, String) {
        // Serialize the coupon
        let serialized_coupon: String = serde_json::to_string(self).unwrap();

        // Hash the serialized coupon using SHA-256
        let mut hasher = Sha256::new();
        hasher.update(serialized_coupon.clone());
        let hashed_coupon = hasher.finalize().to_vec();

        let args = SignWithEcdsaArgument {
            message_hash: hashed_coupon,
            derivation_path: DERIVATION_PATH.into_iter().map(|x| x.to_vec()).collect(),
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: crate::state::read_state(|s| s.ecdsa_key_name.clone()),
            },
        };
        let response: Result<(SignWithEcdsaResponse,), (RejectionCode, String)> =
            sign_with_ecdsa(args).await;

        match response {
            Ok(res) => (serialized_coupon, hex::encode(&res.0.signature)),
            Err((code, msg)) => {
                panic!("Failed to sign_with_ecdsa: {:?}", (code, msg));
            }
        }
    }
}
