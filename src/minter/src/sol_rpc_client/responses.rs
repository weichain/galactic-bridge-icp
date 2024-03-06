use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub result: Vec<T>,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct SignatureResponse {
    #[serde(rename = "blockTime")]
    pub block_time: u64,
    #[serde(rename = "confirmationStatus")]
    pub confirmation_status: String,
    pub err: Option<String>,
    pub memo: Option<String>,
    pub signature: String,
    pub slot: u64,
}
