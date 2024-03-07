// This constant is our approximation of the expected header size.
// The HTTP standard doesn't define any limit, and many implementations limit
// the headers size to 8 KiB. We chose a lower limit because headers observed on most providers
// fit in the constant defined below, and if there is spike, then the payload size adjustment
// should take care of that.
pub const HEADER_SIZE_LIMIT: u64 = 2 * 1024;

// This constant comes from the IC specification:
// > If provided, the value must not exceed 2MB
pub const HTTP_MAX_SIZE: u64 = 2_000_000;

pub const MAX_PAYLOAD_SIZE: u64 = HTTP_MAX_SIZE - HEADER_SIZE_LIMIT;

// In case no memo is set signature object should be around 175 bytes long.
pub const SIGNATURE_RESPONSE_SIZE_ESTIMATE: u64 = 250;

// In case no memo is set transaction object should be around 1100 bytes long.
pub const TRANSACTION_RESPONSE_SIZE_ESTIMATE: u64 = 1500;

#[derive(Debug, Clone, Copy)]
pub enum RpcMethod {
    GetSignaturesForAddress,
    GetTransaction,
}

impl RpcMethod {
    pub fn as_str(&self) -> String {
        match self {
            RpcMethod::GetSignaturesForAddress => "getSignaturesForAddress".to_string(),
            RpcMethod::GetTransaction => "getTransaction".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConfirmationStatus {
    Finalized,
    Confirmed,
    Processed,
}

impl ConfirmationStatus {
    pub fn as_str(&self) -> String {
        match self {
            ConfirmationStatus::Finalized => "finalized".to_string(),
            ConfirmationStatus::Confirmed => "confirmed".to_string(),
            ConfirmationStatus::Processed => "processed".to_string(),
        }
    }
}
