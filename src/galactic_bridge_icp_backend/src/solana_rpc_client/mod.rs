use crate::eth_rpc::{
    self, are_errors_consistent, Block, BlockSpec, FeeHistory, FeeHistoryParams, GetLogsParam,
    Hash, HttpOutcallError, HttpOutcallResult, HttpResponsePayload, JsonRpcResult, LogEntry,
    ResponseSizeEstimate, SendRawTransactionResult,
};
use crate::lifecycle::SolanaNetwork;
use crate::logs::{DEBUG, INFO};
use crate::numeric::TransactionCount;
use crate::solana_rpc_client::providers::{RpcNodeProvider, MAINNET_PROVIDERS, TESTNET_PROVIDERS};
use crate::solana_rpc_client::requests::GetTransactionCountParams;
use crate::state::State;
use ic_canister_log::log;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;

pub mod errors;
mod providers;
pub mod requests;
pub mod responses;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolanaRpcClient {
    chain: SolanaNetwork,
}

impl SolanaRpcClient {
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

    /// Query all providers in parallel and return all results.
    /// It's up to the caller to decide how to handle the results, which could be inconsistent among one another,
    /// (e.g., if different providers gave different responses).
    /// This method is useful for querying data that is critical for the system to ensure that there is no single point of failure,
    async fn parallel_call<I, O>(
        &self,
        method: impl Into<String> + Clone,
        params: I,
        response_size_estimate: ResponseSizeEstimate,
    ) -> MultiCallResults<O>
    where
        I: Serialize + Clone,
        O: DeserializeOwned + HttpResponsePayload,
    {
        let providers = self.providers();
        let results = {
            let mut fut = Vec::with_capacity(providers.len());
            for provider in providers {
                log!(DEBUG, "[parallel_call]: will call provider: {:?}", provider);
                fut.push(eth_rpc::call(
                    provider.url().to_string(),
                    method.clone(),
                    params.clone(),
                    response_size_estimate,
                ));
            }
            futures::future::join_all(fut).await
        };
        MultiCallResults::from_non_empty_iter(providers.iter().cloned().zip(results.into_iter()))
    }

    pub async fn get_signatures_for_address(
        &self,
        params: requests::GetSignaturesForAddressRequest,
    ) -> Result<
        Option<responses::RpcConfirmedTransactionStatusWithSignature>,
        MultiCallError<Option<responses::RpcConfirmedTransactionStatusWithSignature>>,
    > {
        let results: MultiCallResults<responses::RpcConfirmedTransactionStatusWithSignature> = self
            .parallel_call(
                "getSignaturesForAddress",
                params,
                ResponseSizeEstimate::new(512),
            )
            .await;
        results.reduce_with_equality()
    }
}

/// Aggregates responses of different providers to the same query.
/// Guaranteed to be non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiCallResults<T> {
    results: BTreeMap<RpcNodeProvider, HttpOutcallResult<JsonRpcResult<T>>>,
}

impl<T> MultiCallResults<T> {
    fn from_non_empty_iter<
        I: IntoIterator<Item = (RpcNodeProvider, HttpOutcallResult<JsonRpcResult<T>>)>,
    >(
        iter: I,
    ) -> Self {
        let results = BTreeMap::from_iter(iter);
        if results.is_empty() {
            panic!("BUG: MultiCallResults cannot be empty!")
        }
        Self { results }
    }
}

impl<T: PartialEq> MultiCallResults<T> {
    /// Expects all results to be ok or return the following error:
    /// * MultiCallError::ConsistentJsonRpcError: all errors are the same JSON-RPC error.
    /// * MultiCallError::ConsistentHttpOutcallError: all errors are the same HTTP outcall error.
    /// * MultiCallError::InconsistentResults if there are different errors.
    fn all_ok(self) -> Result<BTreeMap<RpcNodeProvider, T>, MultiCallError<T>> {
        let mut results = BTreeMap::new();
        let mut first_error: Option<(RpcNodeProvider, HttpOutcallResult<JsonRpcResult<T>>)> = None;
        for (provider, result) in self.results.into_iter() {
            match result {
                Ok(JsonRpcResult::Result(value)) => {
                    results.insert(provider, value);
                }
                _ => match first_error {
                    None => {
                        first_error = Some((provider, result));
                    }
                    Some((first_error_provider, error)) => {
                        if !are_errors_consistent(&error, &result) {
                            return Err(MultiCallError::InconsistentResults(
                                MultiCallResults::from_non_empty_iter(vec![
                                    (first_error_provider, error),
                                    (provider, result),
                                ]),
                            ));
                        }
                        first_error = Some((first_error_provider, error));
                    }
                },
            }
        }
        match first_error {
            None => Ok(results),
            Some((_provider, Ok(JsonRpcResult::Error { code, message }))) => {
                Err(MultiCallError::ConsistentJsonRpcError { code, message })
            }
            Some((_provider, Err(error))) => Err(MultiCallError::ConsistentHttpOutcallError(error)),
            Some((_, Ok(JsonRpcResult::Result(_)))) => {
                panic!("BUG: first_error should be an error type")
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MultiCallError<T> {
    ConsistentHttpOutcallError(HttpOutcallError),
    ConsistentJsonRpcError { code: i64, message: String },
    InconsistentResults(MultiCallResults<T>),
}

impl<T> MultiCallError<T> {
    pub fn has_http_outcall_error_matching<P: Fn(&HttpOutcallError) -> bool>(
        &self,
        predicate: P,
    ) -> bool {
        match self {
            MultiCallError::ConsistentHttpOutcallError(error) => predicate(error),
            MultiCallError::ConsistentJsonRpcError { .. } => false,
            MultiCallError::InconsistentResults(results) => {
                results.results.values().any(|result| match result {
                    Ok(JsonRpcResult::Result(_)) => false,
                    Ok(JsonRpcResult::Error { .. }) => false,
                    Err(error) => predicate(error),
                })
            }
        }
    }
}

impl<T: Debug + PartialEq> MultiCallResults<T> {
    pub fn reduce_with_equality(self) -> Result<T, MultiCallError<T>> {
        let mut results = self.all_ok()?.into_iter();
        let (base_node_provider, base_result) = results
            .next()
            .expect("BUG: MultiCallResults is guaranteed to be non-empty");
        let mut inconsistent_results: Vec<_> = results
            .filter(|(_provider, result)| result != &base_result)
            .collect();
        if !inconsistent_results.is_empty() {
            inconsistent_results.push((base_node_provider, base_result));
            let error = MultiCallError::InconsistentResults(MultiCallResults::from_non_empty_iter(
                inconsistent_results
                    .into_iter()
                    .map(|(provider, result)| (provider, Ok(JsonRpcResult::Result(result)))),
            ));
            log!(
                INFO,
                "[reduce_with_equality]: inconsistent results {error:?}"
            );
            return Err(error);
        }
        Ok(base_result)
    }

    pub fn reduce_with_min_by_key<F: FnMut(&T) -> K, K: Ord>(
        self,
        extractor: F,
    ) -> Result<T, MultiCallError<T>> {
        let min = self
            .all_ok()?
            .into_values()
            .min_by_key(extractor)
            .expect("BUG: MultiCallResults is guaranteed to be non-empty");
        Ok(min)
    }

    pub fn reduce_with_strict_majority_by_key<F: Fn(&T) -> K, K: Ord>(
        self,
        extractor: F,
    ) -> Result<T, MultiCallError<T>> {
        let mut votes_by_key: BTreeMap<K, BTreeMap<RpcNodeProvider, T>> = BTreeMap::new();
        for (provider, result) in self.all_ok()?.into_iter() {
            let key = extractor(&result);
            match votes_by_key.remove(&key) {
                Some(mut votes_for_same_key) => {
                    let (_other_provider, other_result) = votes_for_same_key
                        .last_key_value()
                        .expect("BUG: results_with_same_key is non-empty");
                    if &result != other_result {
                        let error = MultiCallError::InconsistentResults(
                            MultiCallResults::from_non_empty_iter(
                                votes_for_same_key
                                    .into_iter()
                                    .chain(std::iter::once((provider, result)))
                                    .map(|(provider, result)| {
                                        (provider, Ok(JsonRpcResult::Result(result)))
                                    }),
                            ),
                        );
                        log!(
                            INFO,
                            "[reduce_with_strict_majority_by_key]: inconsistent results {error:?}"
                        );
                        return Err(error);
                    }
                    votes_for_same_key.insert(provider, result);
                    votes_by_key.insert(key, votes_for_same_key);
                }
                None => {
                    let _ = votes_by_key.insert(key, BTreeMap::from([(provider, result)]));
                }
            }
        }

        let mut tally: Vec<(K, BTreeMap<RpcNodeProvider, T>)> = Vec::from_iter(votes_by_key);
        tally.sort_unstable_by(|(_left_key, left_ballot), (_right_key, right_ballot)| {
            left_ballot.len().cmp(&right_ballot.len())
        });
        match tally.len() {
            0 => panic!("BUG: tally should be non-empty"),
            1 => Ok(tally
                .pop()
                .and_then(|(_key, mut ballot)| ballot.pop_last())
                .expect("BUG: tally is non-empty")
                .1),
            _ => {
                let mut first = tally.pop().expect("BUG: tally has at least 2 elements");
                let second = tally.pop().expect("BUG: tally has at least 2 elements");
                if first.1.len() > second.1.len() {
                    Ok(first
                        .1
                        .pop_last()
                        .expect("BUG: tally should be non-empty")
                        .1)
                } else {
                    let error =
                        MultiCallError::InconsistentResults(MultiCallResults::from_non_empty_iter(
                            first
                                .1
                                .into_iter()
                                .chain(second.1)
                                .map(|(provider, result)| {
                                    (provider, Ok(JsonRpcResult::Result(result)))
                                }),
                        ));
                    log!(
                        INFO,
                        "[reduce_with_strict_majority_by_key]: no strict majority {error:?}"
                    );
                    Err(error)
                }
            }
        }
    }
}
