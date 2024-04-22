use crate::{
    constants::{
        MINT_GSOL_RETRY_LIMIT, SOLANA_SIGNATURE_RANGES_RETRY_LIMIT, SOLANA_SIGNATURE_RETRY_LIMIT,
    },
    events::{DepositEvent, SolanaSignature, SolanaSignatureRange},
    guard::TimerGuard,
    logs::{DEBUG, INFO},
    sol_rpc_client::{responses::GetTransactionResponse, LedgerMemo, SolRpcClient, SolRpcError},
    state::audit::process_event,
    state::event::EventType,
    state::{mutate_state, read_state, State, TaskType},
    utils::{HashMapUtils, VecUtils},
};

use icrc_ledger_types::icrc1::transfer::TransferError;
use num_traits::ToPrimitive;
use std::collections::HashMap;

const GET_SIGNATURES_BY_ADDRESS_LIMIT: u8 = 10;
const GET_TRANSACTIONS_LIMIT: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositError {
    RpcCallFailed(SolRpcError),
    SignatureFailed { sig: String, err: SolRpcError },
    SignatureNotFound(String),
    InvalidDepositData(String),
    NonDepositTransaction(String),
    MintingGSolFailed(TransferError),
    SendingMessageToLedgerFailed { id: String, code: i32, msg: String },
}

impl std::fmt::Display for DepositError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepositError::RpcCallFailed(err) => {
                write!(f, "{err:?}")
            }
            DepositError::SignatureFailed { sig, err } => {
                write!(f, "Signature {sig} : failed with {err:?}")
            }
            DepositError::SignatureNotFound(sig) => {
                write!(f, "Signature {sig} : transaction not found")
            }
            DepositError::InvalidDepositData(sig) => {
                write!(f, "Signature {sig} : invalid deposit data")
            }
            DepositError::NonDepositTransaction(sig) => {
                write!(f, "Signature {sig} : non-Deposit transaction found")
            }
            DepositError::MintingGSolFailed(err) => {
                write!(f, "Failed to mint gSOL: {err:?}")
            }
            DepositError::SendingMessageToLedgerFailed { id, code, msg } => {
                write!(
                    f,
                    "Failed to send a message to the ledger {id}: {code:?}: {msg}",
                )
            }
        }
    }
}

// fetch newest signature and push a new range to the state
pub async fn get_latest_signature() {
    let _guard = match TimerGuard::new(TaskType::GetLatestSignature) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    ic_canister_log::log!(DEBUG, "\nSearching for new signatures ...");

    let until_signature = read_state(|s| s.get_solana_last_known_signature());

    // RPC call underneath is exclusive, so until_signature is not included in the result
    match read_state(SolRpcClient::from_state)
        .get_signatures_for_address(1, None, &until_signature)
        .await
    {
        Ok(signatures) => match signatures.len() {
            0 => {
                ic_canister_log::log!(DEBUG, "\nNo new signatures found");
            }
            1 => {
                let newest_sig = signatures[0].signature.to_string();
                process_new_solana_signature_range(&newest_sig, &until_signature);
            }
            _ => {
                ic_canister_log::log!(INFO, "\nUnexpected behaviour");
            }
        },
        Err(error) => {
            ic_canister_log::log!(INFO, "\nFailed to get signatures for address: {error:?}");
        }
    }
}

pub async fn scrap_signature_range() {
    let _guard = match TimerGuard::new(TaskType::ScrapSignatureRanges) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);
    // filter out all events that have reached the retry limit
    let filtered_ranges =
        HashMapUtils::filter(&read_state(|s| s.solana_signature_ranges.clone()), |s| {
            !s.retry
                .is_retry_limit_reached(SOLANA_SIGNATURE_RANGES_RETRY_LIMIT)
        });

    ic_canister_log::log!(
        DEBUG,
        "\nProcessing ranges:\n{}",
        HashMapUtils::format_keys_as_string(&filtered_ranges)
    );

    for (_, v) in &filtered_ranges {
        process_signature_range_with_limit(&rpc_client, v.clone(), None).await;
    }
}

async fn process_signature_range_with_limit(
    rpc_client: &SolRpcClient,
    range: SolanaSignatureRange,
    limit: Option<u8>,
) {
    let limit = limit.unwrap_or(GET_SIGNATURES_BY_ADDRESS_LIMIT);
    let mut before_signature = range.before_sol_sig.to_string();
    let until_signature = range.until_sol_sig.to_string();

    let mut result: Vec<String> = Vec::new();
    let mut at_least_one_successful_call = false; // Flag to track if at least one call was successful

    loop {
        ic_canister_log::log!(
            DEBUG,
            "\nScanning range:\n\tbefore: {before_signature}\n\tuntil: {until_signature}\n\tlimit: {limit}",
        );

        // get signatures for chunk
        match rpc_client
            .get_signatures_for_address(limit, Some(&before_signature), &until_signature)
            .await
        {
            Ok(signatures) => {
                // If at least one call was successful, add the initial element.
                // Call is non inclusive, so we need to add the first element only once.
                if !at_least_one_successful_call {
                    result.push(before_signature.to_string());
                    at_least_one_successful_call = true;
                }

                // if no signatures are available, we are done
                if signatures.is_empty() {
                    remove_solana_signature_range(&range);
                    break;
                }

                // if signatures are available, we continue with the next chunk
                // store the last signature to use it as before for the next chunk
                let last_signature = signatures.last().unwrap();
                before_signature = last_signature.signature.to_string();
                result.extend(signatures.iter().map(|s| s.signature.to_string()));
            }
            Err(error) => {
                // if RPC call failed to get signatures, retry later
                process_retry_solana_signature_range(
                    &range,
                    &before_signature,
                    &until_signature,
                    DepositError::RpcCallFailed(error),
                );

                break;
            }
        }
    }

    // Only process the signatures if at least one successful call was made
    if at_least_one_successful_call {
        result
            .iter()
            .for_each(|s| process_solana_signature(&SolanaSignature::new(s.to_string()), None));
    }
}

pub async fn scrap_signatures() {
    let _guard = match TimerGuard::new(TaskType::ScrapSignatures) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);
    // filter out all events that have reached the retry limit
    let filtered_signatures =
        HashMapUtils::filter(&read_state(|s| s.solana_signatures.clone()), |s| {
            !s.retry.is_retry_limit_reached(SOLANA_SIGNATURE_RETRY_LIMIT)
        });

    ic_canister_log::log!(
        DEBUG,
        "\nProcessing signatures:\n{}",
        HashMapUtils::format_keys_as_string(&filtered_signatures)
    );

    let transactions = process_signatures_with_limit(&rpc_client, &filtered_signatures, None).await;

    ic_canister_log::log!(
        DEBUG,
        "\nProcessing transactions:\n{}",
        VecUtils::format_keys_as_string(&transactions)
    );

    parse_log_messages(&transactions);
}

async fn process_signatures_with_limit(
    rpc_client: &SolRpcClient,
    signatures_map: &HashMap<String, SolanaSignature>,
    limit: Option<u8>,
) -> Vec<(SolanaSignature, GetTransactionResponse)> {
    let limit = limit.unwrap_or(GET_TRANSACTIONS_LIMIT);
    let mut transactions: Vec<(SolanaSignature, GetTransactionResponse)> = Vec::new();

    let signatures: Vec<&SolanaSignature> = signatures_map.values().collect();
    for chunk in signatures.chunks(limit as usize) {
        let signatures = chunk.iter().map(|elem| &elem.sol_sig).collect();

        match rpc_client.get_transactions(signatures).await {
            Ok(txs) => {
                for (key, value) in txs {
                    let signature = signatures_map.get(&key).unwrap().clone();

                    match value {
                        Err(err) => {
                            process_solana_signature(
                                &signature,
                                Some(DepositError::SignatureFailed { sig: key, err }),
                            );
                        }
                        Ok(None) => {
                            process_solana_signature(
                                &signature,
                                Some(DepositError::SignatureNotFound(key)),
                            );
                        }
                        Ok(Some(tx)) => {
                            transactions.push((signature, tx));
                        }
                    }
                }
            }
            Err(err) => {
                // if RPC call failed to get transactions, skip the transactions and retry later
                chunk.iter().for_each(|s| {
                    process_solana_signature(*s, Some(DepositError::RpcCallFailed(err.clone())))
                });
            }
        };
    }

    return transactions;
}

fn parse_log_messages(transactions: &Vec<(SolanaSignature, GetTransactionResponse)>) {
    for (signature, transaction) in transactions {
        match process_transaction_logs(transaction) {
            Ok(deposit) => {
                process_accepted_event(&deposit, None);
            }
            Err(error) => {
                process_invalid_event(signature, error);
            }
        };
    }
}

fn process_transaction_logs(
    transaction: &GetTransactionResponse,
) -> Result<DepositEvent, DepositError> {
    let deposit_msg = "Program log: Instruction: Deposit";
    let success_msg = &format!(
        "Program {} success",
        read_state(|s| s.solana_contract_address.clone())
    );
    let program_data_msg = "Program data: ";

    let signature = &transaction.transaction.signatures[0];
    let solana_address = &transaction.transaction.message.account_keys[0];
    let msgs = &transaction.meta.log_messages;

    if msgs.contains(&String::from(deposit_msg))
        && msgs.contains(&String::from(success_msg))
        && msgs.iter().any(|s| s.starts_with(program_data_msg))
    {
        if let Some(program_data) = msgs.iter().find(|s| s.starts_with(program_data_msg)) {
            let base64_data = program_data.trim_start_matches(program_data_msg);
            let deposit: DepositEvent = DepositEvent::new(
                mutate_state(State::next_deposit_id),
                signature.as_str(),
                solana_address.as_str(),
                base64_data,
            );

            return Ok(deposit);
        } else {
            return Err(DepositError::InvalidDepositData(signature.to_string()));
        }
    } else {
        return Err(DepositError::NonDepositTransaction(signature.to_string()));
    }
}

pub async fn mint_gsol() {
    use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
    use icrc_ledger_types::icrc1::{account::Account, transfer::TransferArg};

    let _guard = match TimerGuard::new(TaskType::MintGSol) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let ledger_canister_id = read_state(|s| s.ledger_id);
    // filter out all events that have reached the retry limit
    let filtered_events = HashMapUtils::filter(&read_state(|s| s.accepted_events.clone()), |e| {
        !e.retry.is_retry_limit_reached(MINT_GSOL_RETRY_LIMIT)
    });

    ic_canister_log::log!(
        DEBUG,
        "\nMinting gSOL:\n{}",
        HashMapUtils::format_keys_as_string(&filtered_events)
    );

    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id,
    };

    for (_, mut event) in filtered_events {
        match client
            .transfer(TransferArg {
                from_subaccount: None,
                to: Account {
                    owner: event.to_icp_address,
                    subaccount: None,
                },
                amount: event.amount.clone(),
                fee: None,
                created_at_time: Some(ic_cdk::api::time()),
                // Memo is limited to 32 bytes in size, so can't fit much in there
                memo: Some(LedgerMemo(event.id).into()),
            })
            .await
        {
            Ok(Ok(block_index)) => {
                let block_index = block_index.0.to_u64().expect("nat does not fit into u64");
                event.update_mint_block_index(block_index);
                process_minted_event(&event);
            }
            Ok(Err(err)) => {
                process_accepted_event(&event, Some(DepositError::MintingGSolFailed(err.clone())));
            }
            Err(err) => {
                process_accepted_event(
                    &event,
                    Some(DepositError::SendingMessageToLedgerFailed {
                        id: ledger_canister_id.to_string(),
                        code: err.0,
                        msg: err.1,
                    }),
                );
            }
        };
    }
}

/// Process events
fn process_minted_event(event: &DepositEvent) {
    ic_canister_log::log!(
        DEBUG,
        "\nProcessed Signature: {}\n\tMinted amount: {}\n\tto {}\n\tin block {}",
        event.sol_sig,
        event.amount,
        event.to_icp_address,
        event.get_mint_block_index().unwrap()
    );

    mutate_state(|s| {
        process_event(
            s,
            EventType::MintedEvent {
                event_source: event.clone(),
            },
        )
    });
}

fn process_accepted_event(event: &DepositEvent, err: Option<DepositError>) {
    if let Some(err) = err.clone() {
        ic_canister_log::log!(DEBUG, "{err}");
    } else {
        ic_canister_log::log!(
            DEBUG,
            "\nSignature {} : Deposit transaction found",
            event.sol_sig
        );
    }

    mutate_state(|s| {
        process_event(
            s,
            EventType::AcceptedEvent {
                event_source: event.clone(),
                fail_reason: err.map(|e| e.to_string()),
            },
        )
    });
}

fn process_invalid_event(signature: &SolanaSignature, err: DepositError) {
    ic_canister_log::log!(DEBUG, "\nSignature {} : {err}", signature.sol_sig);

    mutate_state(|s| {
        process_event(
            s,
            EventType::InvalidEvent {
                signature: signature.clone(),
                fail_reason: err.to_string(),
            },
        );
    });
}

fn process_solana_signature(signature: &SolanaSignature, err: Option<DepositError>) {
    if let Some(err) = err.clone() {
        ic_canister_log::log!(DEBUG, "{err}");
    } else {
        ic_canister_log::log!(
            INFO,
            "\nSignature {} : Transaction found",
            signature.sol_sig
        );
    }

    mutate_state(|s| {
        process_event(
            s,
            EventType::SolanaSignature {
                signature: signature.clone(),
                fail_reason: err.map(|e| e.to_string()),
            },
        );
    });
}

fn process_new_solana_signature_range(newest_signature: &str, until_signature: &str) {
    ic_canister_log::log!(DEBUG, "\nNew signature found: {newest_signature}",);

    mutate_state(|s| {
        process_event(
            s,
            EventType::LastKnownSolanaSignature(newest_signature.to_string()),
        );
        process_event(
            s,
            EventType::NewSolanaSignatureRange(SolanaSignatureRange::new(
                newest_signature.to_string(),
                until_signature.to_string(),
            )),
        );
    });
}

fn process_retry_solana_signature_range(
    range: &SolanaSignatureRange,
    before_signature: &str,
    until_signature: &str,
    error: DepositError,
) {
    let error_msg = format!("\nFailed to get signatures for address:\n\tbefore: {before_signature}\n\tuntil: {until_signature}\n\terror: {error:?}");
    ic_canister_log::log!(DEBUG, "{error_msg}");

    mutate_state(|s| {
        process_event(
            s,
            EventType::RetrySolanaSignatureRange {
                range: range.clone(),
                failed_sub_range: Some(SolanaSignatureRange::new(
                    before_signature.to_string(),
                    until_signature.to_string(),
                )),
                fail_reason: error_msg,
            },
        )
    });
}

fn remove_solana_signature_range(range: &SolanaSignatureRange) {
    ic_canister_log::log!(
        DEBUG,
        "\nRange completed:\n\tbefore: {}\n\tuntil: {}",
        range.before_sol_sig,
        range.until_sol_sig,
    );

    mutate_state(|s| {
        process_event(s, EventType::RemoveSolanaSignatureRange(range.clone()));
    });
}
