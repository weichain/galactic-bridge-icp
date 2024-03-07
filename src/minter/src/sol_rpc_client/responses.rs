use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JsonRpcSignatureResponse<T> {
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

#[derive(Debug, Deserialize)]
pub struct JsonRpcTransactionResponse<T> {
    pub jsonrpc: String,
    pub result: T,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub numReadonlySignedAccounts: u64,
    pub numReadonlyUnsignedAccounts: u64,
    pub numRequiredSignatures: u64,
}

#[derive(Debug, Deserialize)]
pub struct Instruction {
    pub accounts: Vec<u64>,
    pub data: String,
    pub programIdIndex: u64,
    pub stackHeight: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub accountKeys: Vec<String>,
    pub header: Header,
    pub instructions: Vec<Instruction>,
    pub recentBlockhash: String,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub computeUnitsConsumed: u64,
    pub err: Option<serde_json::Value>,
    pub fee: u64,
    pub innerInstructions: Vec<serde_json::Value>,
    pub loadedAddresses: LoadedAddresses,
    pub logMessages: Vec<String>,
    pub postBalances: Vec<u64>,
    pub postTokenBalances: Vec<serde_json::Value>,
    pub preBalances: Vec<u64>,
    pub preTokenBalances: Vec<serde_json::Value>,
    pub rewards: Vec<serde_json::Value>,
    pub status: Status,
}

#[derive(Debug, Deserialize)]
pub struct Status {
    pub Ok: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct LoadedAddresses {
    pub readonly: Vec<serde_json::Value>,
    pub writable: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub message: Message,
    pub signatures: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetTransactionResponse {
    pub blockTime: u64,
    pub meta: Meta,
    pub slot: u64,
    pub transaction: Transaction,
}
