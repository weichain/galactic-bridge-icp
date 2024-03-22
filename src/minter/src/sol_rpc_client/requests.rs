use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GetSignaturesForAddressRequestOptions {
    pub limit: Option<u64>,
    pub commitment: Option<String>,
    pub until: Option<String>,
    pub before: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetTransactionRequestOptions {
    pub commitment: Option<String>,
}
