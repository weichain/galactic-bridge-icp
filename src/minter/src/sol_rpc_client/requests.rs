use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GetSignaturesForAddressRequest {
    pub limit: Option<u64>,
    pub commitment: Option<String>,
    pub until: Option<String>,
    pub before: Option<String>,
}
