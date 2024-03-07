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
struct Header {
    numReadonlySignedAccounts: u64,
    numReadonlyUnsignedAccounts: u64,
    numRequiredSignatures: u64,
}

#[derive(Debug, Deserialize)]
struct Instruction {
    accounts: Vec<u64>,
    data: String,
    programIdIndex: u64,
    stackHeight: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Message {
    accountKeys: Vec<String>,
    header: Header,
    instructions: Vec<Instruction>,
    recentBlockhash: String,
}

#[derive(Debug, Deserialize)]
struct Meta {
    computeUnitsConsumed: u64,
    err: Option<serde_json::Value>,
    fee: u64,
    innerInstructions: Vec<serde_json::Value>,
    loadedAddresses: LoadedAddresses,
    logMessages: Vec<String>,
    postBalances: Vec<u64>,
    postTokenBalances: Vec<serde_json::Value>,
    preBalances: Vec<u64>,
    preTokenBalances: Vec<serde_json::Value>,
    rewards: Vec<serde_json::Value>,
    status: Status,
}

#[derive(Debug, Deserialize)]
struct Status {
    Ok: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LoadedAddresses {
    readonly: Vec<serde_json::Value>,
    writable: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Transaction {
    message: Message,
    signatures: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetTransactionResponse {
    blockTime: u64,
    meta: Meta,
    slot: u64,
    transaction: Transaction,
}
