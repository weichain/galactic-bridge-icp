use minicbor::{Decode, Encode};

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

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct Deposit {
    #[n(0)]
    pub address_icp: String,
    #[n(1)]
    pub amount: u64,
}

impl From<&str> for Deposit {
    fn from(s: &str) -> Self {
        use base64::prelude::*;
        let bytes = BASE64_STANDARD.decode(s).unwrap();

        let amount_bytes = &bytes[bytes.len() - 8..];
        let mut amount: u64 = 0;
        for i in 0..8 {
            amount |= (amount_bytes[i] as u64) << (i * 8);
        }

        let address_bytes = &bytes[12..bytes.len() - 8];
        let address_icp = String::from_utf8_lossy(&address_bytes);

        Deposit {
            address_icp: address_icp.to_string(),
            amount,
        }
    }
}
