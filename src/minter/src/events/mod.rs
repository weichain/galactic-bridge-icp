use minicbor::{Decode, Encode};

pub trait Retriable {
    fn get_retries(&self) -> u8;
    fn increment_retries(&mut self);
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SkippedSolSignatureRange {
    #[n(0)]
    pub before_sol_signature: String,
    #[n(1)]
    pub until_sol_signature: String,
    #[n(2)]
    retries: u8,
}

impl SkippedSolSignatureRange {
    // Constructor function to create a new SkippedSolSignatureRange
    pub fn new(before: String, until: String) -> Self {
        SkippedSolSignatureRange {
            before_sol_signature: before,
            until_sol_signature: until,
            retries: 0,
        }
    }
}

impl Retriable for SkippedSolSignatureRange {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct SkippedSolTransaction {
    #[n(0)]
    pub sol_signature: String,
    #[n(1)]
    retries: u8,
}

impl SkippedSolTransaction {
    // Constructor function to create a new SkippedSolTransaction
    pub fn new(signature: String) -> Self {
        SkippedSolTransaction {
            sol_signature: signature,
            retries: 0,
        }
    }
}

impl Retriable for SkippedSolTransaction {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }
}

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct InvalidSolTransaction {
    #[n(0)]
    pub sol_signature: String,
}

impl InvalidSolTransaction {
    // Constructor function to create a new InvalidSolTransaction
    pub fn new(signature: String) -> Self {
        InvalidSolTransaction {
            sol_signature: signature,
        }
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

#[derive(Debug, Encode, Decode, PartialEq, Clone, Eq)]
pub struct DepositEvent {
    #[n(0)]
    pub deposit: Deposit,
    #[n(1)]
    pub sol_signature: String,
}
