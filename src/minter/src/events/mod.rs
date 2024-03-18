use candid::Principal;
use ic_stable_structures::Storable;
use icrc_ledger_types::icrc1::transfer::Memo;
use minicbor::{Decode, Encode};
use serde::Serialize;

pub trait Retriable {
    fn get_retries(&self) -> u8;
    fn increment_retries(&mut self);
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
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct ReceivedSolEvent {
    #[n(0)]
    pub sol_sig: String,
    #[n(1)]
    pub from_address: String,
    #[n(2)]
    pub value: u64,
    #[cbor(n(3), with = "crate::cbor::principal")]
    pub to: Principal,
    #[n(4)]
    retries: u8,
}

impl Retriable for ReceivedSolEvent {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
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
            sol_sig: sol_sig.to_string(),
            from_address: from_address.to_string(),
            value,
            to: principal,
            retries: 0,
        }
    }
}
