use serde::{Deserialize, Serialize};

/// An envelope for all JSON-RPC requests.
#[derive(Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub method: String,
    pub id: u64,
    pub params: T,
}

#[derive(Serialize, Deserialize)]
pub struct GetSignaturesForAddressRequest {
    pub limit: Option<u64>,
    pub commitment: Option<String>,
    pub until: Option<String>,
    pub before: Option<String>,
}
