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

#[derive(Debug, Clone, Copy)]
pub enum RpcMethod {
    GetSignaturesForAddress,
    // OtherMethod,
    // Add more RPC methods as needed
}

impl RpcMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            RpcMethod::GetSignaturesForAddress => "getSignaturesForAddress",
            // RpcMethod::OtherMethod => "otherMethod",
            // Add more method names mapping as needed
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
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfirmationStatus::Finalized => "finalized",
            ConfirmationStatus::Confirmed => "confirmed",
            ConfirmationStatus::Processed => "processed",
        }
    }
}
