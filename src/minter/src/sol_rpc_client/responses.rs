use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct Header {
    #[serde(rename = "numReadonlySignedAccounts")]
    pub num_readonly_signed_accounts: u64,
    #[serde(rename = "numReadonlyUnsignedAccounts")]
    pub num_readonly_unsigned_accounts: u64,
    #[serde(rename = "numRequiredSignatures")]
    pub num_required_signatures: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Instruction {
    pub accounts: Vec<u64>,
    pub data: String,
    #[serde(rename = "programIdIndex")]
    pub program_id_index: u64,
    #[serde(rename = "stackHeight")]
    pub stack_height: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Message {
    #[serde(rename = "accountKeys")]
    pub account_keys: Vec<String>,
    pub header: Header,
    pub instructions: Vec<Instruction>,
    #[serde(rename = "recentBlockhash")]
    pub recent_blockhash: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meta {
    #[serde(rename = "computeUnitsConsumed")]
    pub compute_units_consumed: u64,
    pub err: Option<serde_json::Value>,
    pub fee: u64,
    #[serde(rename = "innerInstructions")]
    pub inner_instructions: Vec<serde_json::Value>,
    #[serde(rename = "loadedAddresses")]
    pub loaded_addresses: LoadedAddresses,
    #[serde(rename = "logMessages")]
    pub log_messages: Vec<String>,
    #[serde(rename = "postBalances")]
    pub post_balances: Vec<u64>,
    #[serde(rename = "postTokenBalances")]
    pub post_token_balances: Vec<serde_json::Value>,
    #[serde(rename = "preBalances")]
    pub pre_balances: Vec<u64>,
    #[serde(rename = "preTokenBalances")]
    pub pre_token_balances: Vec<serde_json::Value>,
    pub rewards: Vec<serde_json::Value>,
    pub status: Status,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Status {
    #[serde(rename = "Ok")]
    pub ok: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoadedAddresses {
    pub readonly: Vec<serde_json::Value>,
    pub writable: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Transaction {
    pub message: Message,
    pub signatures: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetTransactionResponse {
    #[serde(rename = "blockTime")]
    pub block_time: u64,
    pub meta: Meta,
    pub slot: u64,
    pub transaction: Transaction,
}
