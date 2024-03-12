use crate::lifecycle::SolanaNetwork;
use crate::sol_rpc_client::providers::{RpcNodeProvider, MAINNET_PROVIDERS, TESTNET_PROVIDERS};
use crate::sol_rpc_client::requests::GetSignaturesForAddressRequest;
use crate::sol_rpc_client::responses::{
    GetTransactionResponse, JsonRpcResponse, SignatureResponse,
};
use crate::sol_rpc_client::types::{
    ConfirmationStatus, RpcMethod, HEADER_SIZE_LIMIT, SIGNATURE_RESPONSE_SIZE_ESTIMATE,
    TRANSACTION_RESPONSE_SIZE_ESTIMATE,
};
use crate::state::{read_state, State};

use ic_cdk::api::{
    call::RejectionCode,
    management_canister::http_request::{
        http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
    },
};
use serde_json::json;
use std::collections::HashMap;

mod providers;
pub mod requests;
pub mod responses;
pub mod types;

// TODO: support for multiple providers
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolRpcClient {
    chain: SolanaNetwork,
}

#[derive(Debug)]
pub enum SolRcpError {
    RequestFail(String),
    JsonRpcFail(String),
    FromUtf8Fail(String),
    FromStringOfJsonFail(String),
    ToStringOfJsonFail(String),
}

impl SolRcpError {
    pub fn new_request_fail(code: RejectionCode, msg: &str) -> Self {
        SolRcpError::RequestFail(format!(
            "The http_request resulted into error. RejectionCode: {code:?}, Error: {msg}",
        ))
    }

    pub fn new_json_rpc_fail(code: i32, msg: &str) -> Self {
        SolRcpError::JsonRpcFail(format!(
            "Json response contains error. Code: {code:?}, Error: {msg}",
        ))
    }

    pub fn new_from_utf8_fail(err: &str) -> Self {
        SolRcpError::FromUtf8Fail(format!("FromUtf8Error. {}", err))
    }

    pub fn new_from_string_of_json_fail(err: &str) -> Self {
        SolRcpError::ToStringOfJsonFail(format!("FromStringOfJsonError. {}", err))
    }

    pub fn new_to_string_of_json_fail(err: &str) -> Self {
        SolRcpError::ToStringOfJsonFail(format!("ToStringOfJsonError. {}", err))
    }
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
                let str_body = String::from_utf8(response.body);

                match str_body {
                    Ok(str_body) => Ok(str_body),
                    Err(error) => Err(SolRcpError::new_from_utf8_fail(&error.to_string())),
                }
            }
            Err((r, m)) => Err(SolRcpError::new_request_fail(r, &m)),
        }
    }

    pub async fn get_signatures_for_address(
        &self,
        limit: u64,
        before: Option<String>,
        until: String,
    ) -> Result<Vec<SignatureResponse>, SolRcpError> {
        let params: [&dyn erased_serde::Serialize; 2] = [
            &read_state(|s| s.solana_contract_address.clone()),
            &GetSignaturesForAddressRequest {
                limit: Some(limit),
                commitment: Some(ConfirmationStatus::Finalized.as_str()),
                before,
                until: Some(until),
            },
        ];

        let payload = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": RpcMethod::GetSignaturesForAddress.as_str(),
            "params": params
        }));
        let payload = if let Err(error) = payload {
            return Err(SolRcpError::new_to_string_of_json_fail(&error.to_string()));
        } else {
            payload.unwrap()
        };

        // The effective size estimate is the size of the response we expect to get from the RPC
        let effective_size_estimate: u64 =
            limit * SIGNATURE_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(payload, effective_size_estimate).await {
            Ok(response) => {
                let json_response =
                    serde_json::from_str::<JsonRpcResponse<Vec<SignatureResponse>>>(&response);

                // Check if the response is valid
                match json_response {
                    Ok(json_response) => {
                        // In case error is present in the response ignore the result and return the error
                        if let Some(error) = json_response.error {
                            Err(SolRcpError::new_json_rpc_fail(error.code, &error.message))
                        } else {
                            Ok(json_response.result.unwrap())
                        }
                    }
                    Err(error) => {
                        return Err(SolRcpError::new_from_string_of_json_fail(
                            &error.to_string(),
                        ))
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }

    pub async fn get_transactions(
        &self,
        signatures: Vec<String>,
    ) -> Result<HashMap<String, Result<Option<GetTransactionResponse>, SolRcpError>>, SolRcpError>
    {
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

        let payload = serde_json::to_string(&rpc_request);
        let payload = if let Err(error) = payload {
            return Err(SolRcpError::new_to_string_of_json_fail(&error.to_string()));
        } else {
            payload.unwrap()
        };

        // The effective size estimate is the size of the response we expect to get from the RPC
        let effective_size_estimate: u64 =
            (signatures.len() as u64) * TRANSACTION_RESPONSE_SIZE_ESTIMATE + HEADER_SIZE_LIMIT;

        match self.rpc_call(payload, effective_size_estimate).await {
            Ok(response) => {
                let json_responses =
                    serde_json::from_str::<Vec<JsonRpcResponse<GetTransactionResponse>>>(&response);

                match json_responses {
                    Ok(responses) => {
                        let mut map = HashMap::<
                            String,
                            Result<Option<GetTransactionResponse>, SolRcpError>,
                        >::new();

                        responses
                            .into_iter()
                            .enumerate()
                            .for_each(|(index, response)| {
                                // In case error is present in the response ignore the result and return the error
                                let result = if let Some(error) = response.error {
                                    Err(SolRcpError::new_json_rpc_fail(error.code, &error.message))
                                } else {
                                    Ok(response.result)
                                };

                                map.insert(signatures[index].clone(), result);
                            });

                        Ok(map)
                    }
                    Err(error) => Err(SolRcpError::new_from_string_of_json_fail(
                        &error.to_string(),
                    )),
                }
            }
            Err(error) => return Err(error),
        }
    }
}
