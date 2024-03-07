use crate::lifecycle::SolanaNetwork;
use crate::sol_rpc_client::providers::{RpcNodeProvider, MAINNET_PROVIDERS, TESTNET_PROVIDERS};
use crate::sol_rpc_client::requests::GetSignaturesForAddressRequest;
use crate::sol_rpc_client::responses::{
    GetTransactionResponse, JsonRpcSignatureResponse, JsonRpcTransactionResponse, SignatureResponse,
};
use crate::sol_rpc_client::types::{
    ConfirmationStatus, RpcMethod, HEADER_SIZE_LIMIT, SIGNATURE_RESPONSE_SIZE_ESTIMATE,
    TRANSACTION_RESPONSE_SIZE_ESTIMATE,
};
use crate::state::{read_state, State};

use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
};
use serde_json::json;
use std::fmt::Debug;

mod providers;
pub mod requests;
pub mod responses;
pub mod types;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolRpcClient {
    chain: SolanaNetwork,
}

#[derive(Debug)]
pub enum SolRcpError {
    RpcRequestFail(String),
}

impl SolRpcClient {
    const fn new(chain: SolanaNetwork) -> Self {
        Self { chain }
    }

    pub const fn from_state(state: &State) -> Self {
        Self::new(state.solana_network())
    }

    fn providers(&self) -> &[RpcNodeProvider] {
        match self.chain {
            SolanaNetwork::Mainnet => &MAINNET_PROVIDERS,
            SolanaNetwork::Testnet => &TESTNET_PROVIDERS,
        }
    }

    async fn rpc_call(
        &self,
        payload: String,
        effective_size_estimate: u64,
    ) -> Result<String, SolRcpError> {
        // Details of the values used in the following lines can be found here:
        // https://internetcomputer.org/docs/current/developer-docs/production/computation-and-storage-costs
        let base_cycles = 400_000_000u128 + 100_000u128 * (2 * effective_size_estimate as u128);

        const BASE_SUBNET_SIZE: u128 = 13;
        const SUBNET_SIZE: u128 = 34;
        let cycles = base_cycles * SUBNET_SIZE / BASE_SUBNET_SIZE;

        let request = CanisterHttpRequestArgument {
            url: self.providers()[0].url().to_string(),
            max_response_bytes: Some(effective_size_estimate),
            method: HttpMethod::POST,
            headers: vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            }],
            body: Some(payload.as_bytes().to_vec()),
            transform: None,
        };

        match http_request(request, cycles).await {
            Ok((response,)) => {
                let str_body = String::from_utf8(response.body)
                    .expect("Transformed response is not UTF-8 encoded.");

                Ok(str_body)
            }
            Err((r, m)) => Err(SolRcpError::RpcRequestFail(format!(
                "The http_request resulted into error. RejectionCode: {r:?}, Error: {m}"
            ))),
        }
    }

    pub async fn get_signatures_for_address(
        &self,
        limit: u64,
        before: Option<String>,
        until: String,
    ) -> Option<Vec<SignatureResponse>> {
        let params: [&dyn erased_serde::Serialize; 2] = [
            &read_state(|s| s.solana_contract_address.clone()),
            &GetSignaturesForAddressRequest {
                limit: Some(limit),
                commitment: Some(ConfirmationStatus::Finalized.as_str()),
                before,
                until: Some(until),
            },
        ];

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": RpcMethod::GetSignaturesForAddress.as_str(),
            "params": params
        });

        let payload = serde_json::to_string(&rpc_request).unwrap();

        // The effective size estimate is the size of the response we expect to get from the RPC
        let effective_size_estimate: u64 =
            limit * SIGNATURE_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(payload, effective_size_estimate).await {
            Ok(response) => {
                let response: JsonRpcSignatureResponse<SignatureResponse> =
                    serde_json::from_str(&response).expect("Failed to parse response");

                Some(response.result)
            }
            Err(error) => {
                ic_cdk::api::print(format!("{:?}", error));

                None
            }
        }
    }

    pub async fn get_transactions(
        &self,
        signatures: Vec<String>,
    ) -> Option<Vec<GetTransactionResponse>> {
        let mut rpc_request = Vec::new();
        let mut id = 1;

        for signature in &signatures {
            let transaction = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": RpcMethod::GetTransaction.as_str().to_string(),
                "params": [signature]
            });
            rpc_request.push(transaction);
            id += 1;
        }

        let payload = serde_json::to_string(&rpc_request).unwrap();

        // The effective size estimate is the size of the response we expect to get from the RPC
        let effective_size_estimate: u64 =
            (signatures.len() as u64) * TRANSACTION_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(payload, effective_size_estimate).await {
            Ok(response) => {
                let response: Vec<JsonRpcTransactionResponse<GetTransactionResponse>> =
                    serde_json::from_str(&response).expect("Failed to parse response");

                let result = response.into_iter().map(|r| r.result).collect();

                Some(result)
            }
            Err(error) => {
                ic_cdk::api::print(format!("{:?}", error));

                None
            }
        }
    }
}
