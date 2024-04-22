use crate::withdraw::Coupon;

use candid::{Nat, Principal};
use minicbor::{Decode, Encode};
use num_bigint::BigUint;
use serde::Serialize;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct Retriable(#[n(0)] u8);

impl Retriable {
    pub fn get_retries(&self) -> u8 {
        self.0
    }

    pub fn increment_retries(&mut self) {
        self.0 += 1;
    }

    pub fn reset_retries(&mut self) {
        self.0 = 0;
    }

    pub fn is_retry_limit_reached(&self, limit: u8) -> bool {
        self.get_retries() >= limit
    }
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SolanaSignatureRange {
    #[n(0)]
    pub before_sol_sig: String,
    #[n(1)]
    pub until_sol_sig: String,
    #[n(2)]
    pub retry: Retriable,
}

impl SolanaSignatureRange {
    // Constructor function to create a new SolanaSignatureRange
    pub fn new(before: String, until: String) -> Self {
        SolanaSignatureRange {
            before_sol_sig: before,
            until_sol_sig: until,
            retry: Retriable(0),
        }
    }
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SolanaSignature {
    #[n(0)]
    pub sol_sig: String,
    #[n(1)]
    pub retry: Retriable,
}

impl SolanaSignature {
    // Constructor function to create a new SolanaSignature
    pub fn new(signature: String) -> Self {
        SolanaSignature {
            sol_sig: signature,
            retry: Retriable(0),
        }
    }
}

impl std::fmt::Display for SolanaSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sol_sig)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct DepositEvent {
    #[n(0)]
    pub id: u64,
    #[n(1)]
    pub from_sol_address: String,
    #[cbor(n(2), with = "crate::cbor::principal")]
    pub to_icp_address: Principal,
    #[cbor(n(3), with = "crate::cbor::nat")]
    pub amount: Nat,
    #[n(4)]
    pub sol_sig: String,
    #[n(5)]
    icp_mint_block_index: Option<u64>,
    #[n(6)]
    pub retry: Retriable,
}

impl DepositEvent {
    pub fn new(deposit_id: u64, sol_sig: &str, from_address: &str, encode_data: &str) -> Self {
        use base64::prelude::*;

        let bytes = BASE64_STANDARD.decode(encode_data).unwrap();
        let amount_bytes = &bytes[bytes.len() - 8..];
        let mut value: BigUint = BigUint::default(); // Initialize BigUint to 0
        for i in 0..8 {
            let byte_as_u64 = amount_bytes[i] as u64;
            let shifted_value = BigUint::from(byte_as_u64) << (i * 8); // Shifted byte value as BigUint
            value |= &shifted_value;
        }

        let address_bytes = &bytes[12..bytes.len() - 8];
        let address_hex = String::from_utf8_lossy(&address_bytes);
        let principal = Principal::from_text(address_hex).unwrap();

        DepositEvent {
            id: deposit_id,
            from_sol_address: from_address.to_string(),
            to_icp_address: principal,
            amount: Nat::from(value),
            sol_sig: sol_sig.to_string(),
            icp_mint_block_index: None,
            retry: Retriable(0),
        }
    }

    pub fn update_mint_block_index(&mut self, block_index: u64) {
        self.icp_mint_block_index = Some(block_index);
    }

    pub fn get_mint_block_index(&self) -> Option<u64> {
        self.icp_mint_block_index
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Serialize)]
pub struct WithdrawalEvent {
    #[cbor(n(1), with = "crate::cbor::principal")]
    pub from_icp_address: Principal,
    #[n(2)]
    pub to_sol_address: String,
    #[cbor(n(3), with = "crate::cbor::nat")]
    pub amount: Nat,
    #[n(0)]
    burn_id: u64,
    #[n(4)]
    burn_timestamp: Option<u64>,
    #[n(5)]
    icp_burn_block_index: Option<u64>,
    #[n[6]]
    #[serde(skip_serializing)]
    coupon: Option<Coupon>,
    #[n(7)]
    #[serde(skip_serializing)]
    pub retry: Retriable,
}

impl WithdrawalEvent {
    pub fn new(burn_id: u64, from: Principal, to_sol_address: String, amount: Nat) -> Self {
        WithdrawalEvent {
            from_icp_address: from,
            to_sol_address,
            amount,
            burn_id,
            burn_timestamp: None,
            icp_burn_block_index: None,
            coupon: None,
            retry: Retriable(0),
        }
    }

    pub fn get_burn_id(&self) -> u64 {
        self.burn_id
    }

    pub fn update_after_burn(&mut self, timestamp: u64, block_index: u64) {
        self.burn_timestamp = Some(timestamp);
        self.icp_burn_block_index = Some(block_index);
    }

    pub fn update_after_redeem(&mut self, coupon: Coupon) {
        self.coupon = Some(coupon);
    }

    pub fn get_coupon(&self) -> Option<&Coupon> {
        self.coupon.as_ref()
    }
}
