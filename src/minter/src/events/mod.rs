pub trait Retriable {
    fn get_retries(&self) -> u8;
    fn increment_retries(&mut self);
}

#[derive(Debug, PartialEq, Clone)]
pub struct SkippedSignatureRange {
    pub before: String,
    pub until: String,
    pub error: String,
    retries: u8,
}

impl SkippedSignatureRange {
    // Constructor function to create a new SkippedSignatureRange
    pub fn new(before: String, until: String, error: String) -> Self {
        SkippedSignatureRange {
            before,
            until,
            error,
            retries: 0,
        }
    }
}

impl Retriable for SkippedSignatureRange {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SkippedTransaction {
    pub signature: String,
    pub error: String,
    retries: u8,
}

impl SkippedTransaction {
    // Constructor function to create a new SkippedTransaction
    pub fn new(signature: String, error: String) -> Self {
        SkippedTransaction {
            signature,
            error,
            retries: 0,
        }
    }
}

impl Retriable for SkippedTransaction {
    fn get_retries(&self) -> u8 {
        self.retries
    }

    fn increment_retries(&mut self) {
        self.retries += 1;
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct InvalidTransaction {
    pub signature: String,
    pub info: String,
}

impl InvalidTransaction {
    // Constructor function to create a new InvalidTransaction
    pub fn new(signature: String, info: String) -> Self {
        InvalidTransaction { signature, info }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DepositEvent {
    pub address_icp: String,
    pub amount: u64,
}

impl From<&str> for DepositEvent {
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

        DepositEvent {
            address_icp: address_icp.to_string(),
            amount,
        }
    }
}
