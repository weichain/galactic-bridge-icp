use crate::events::{ReceivedSolEvent, SolanaSignature, SolanaSignatureRange};
use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::GetTransactionResponse;
use crate::sol_rpc_client::SolRpcClient;
use crate::state::audit::process_event;
use crate::state::event::EventType;
use crate::state::{mutate_state, read_state, TaskType};

use num_traits::ToPrimitive;
use std::collections::HashMap;

const GET_SIGNATURES_BY_ADDRESS_LIMIT: u64 = 10;
const GET_TRANSACTIONS_LIMIT: u64 = 5;

// fetch newest signature and push a new range to the state
pub async fn get_latest_signature() {
    let _guard = match TimerGuard::new(TaskType::GetLatestSignature) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    ic_canister_log::log!(DEBUG, "Searching for new signatures ...");

    let until_signature = read_state(|s| s.get_solana_last_known_signature());

    // RPC call underneath is exclusive, so until_signature is not included in the result
    match read_state(SolRpcClient::from_state)
        .get_signatures_for_address(1, None, &until_signature)
        .await
    {
        Ok(signatures) => match signatures.len() {
            0 => ic_canister_log::log!(DEBUG, "No new signatures found."),
            1 => {
                let newest_sig = signatures[0].signature.to_string();
                process_new_solana_signature_range(&newest_sig, &until_signature);
            }
            _ => {
                ic_canister_log::log!(DEBUG, "Unexpected behaviour.",);
            }
        },
        Err(error) => {
            ic_canister_log::log!(
                DEBUG,
                "Failed to get signatures for address. Error: {error:?}."
            );
        }
    }
}

pub async fn scrap_signature_range() {
    let _guard = match TimerGuard::new(TaskType::ScrapSignatureRanges) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);
    let ranges_map = read_state(|s| s.solana_signature_ranges.clone());

    for (_, v) in &ranges_map {
        process_signature_range_with_limit(&rpc_client, v.clone(), None).await;
    }
}

async fn process_signature_range_with_limit(
    rpc_client: &SolRpcClient,
    range: SolanaSignatureRange,
    limit: Option<u64>,
) {
    let limit = limit.unwrap_or(GET_SIGNATURES_BY_ADDRESS_LIMIT);
    let mut before_signature = range.before_sol_sig.to_string();
    let until_signature = range.until_sol_sig.to_string();

    loop {
        ic_canister_log::log!(
            DEBUG,
            "Scanning range: before: {before_signature}, until: {until_signature} with limit: {limit} ...",
        );

        // get signatures for chunk
        match rpc_client
            .get_signatures_for_address(limit, Some(&before_signature), &until_signature)
            .await
        {
            Ok(signatures) => {
                // if no signatures are available, we are done
                if signatures.is_empty() {
                    remove_solana_signature_range(&range);
                    break;
                }

                // if signatures are available, we continue with the next chunk
                // store the last signature to use it as before for the next chunk

                // include the first signature, call is not inclusive
                if before_signature == range.before_sol_sig {
                    process_solana_signature(
                        &SolanaSignature::new(before_signature.to_string()),
                        None,
                    )
                }

                let last_signature = signatures.last().unwrap();
                before_signature = last_signature.signature.to_string();

                signatures.iter().for_each(|s| {
                    process_solana_signature(&SolanaSignature::new(s.signature.to_string()), None)
                });
            }
            Err(error) => {
                // if RPC call failed to get signatures, retry later
                process_retry_solana_signature_range(
                    &range,
                    &before_signature,
                    &until_signature,
                    &format!("{error:?}"),
                );

                break;
            }
        }
    }
}

pub async fn scrap_signatures() {
    let _guard = match TimerGuard::new(TaskType::ScrapSignatures) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);
    let signatures_map = &read_state(|s| s.solana_signatures.clone());

    ic_canister_log::log!(DEBUG, " signatures: {:?}", signatures_map.keys());

    let transactions = process_signatures_with_limit(&rpc_client, signatures_map, None).await;

    ic_canister_log::log!(
        DEBUG,
        "Parsing transactions {:?} ...",
        signatures_map.iter().map(|(s, _)| s.to_string())
    );

    parse_log_messages(&transactions);
}

async fn process_signatures_with_limit(
    rpc_client: &SolRpcClient,
    signatures_map: &HashMap<String, SolanaSignature>,
    limit: Option<u64>,
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
                        Err(error) => {
                            let error_msg = format!("Signature: {key} -> Failed with {error:?}.");
                            process_solana_signature(&signature, Some(&error_msg));
                        }
                        Ok(None) => {
                            let error_msg = format!("Signature: {key} -> Transaction not found.");
                            process_solana_signature(&signature, Some(&error_msg));
                        }
                        Ok(Some(tx)) => {
                            transactions.push((signature, tx));
                        }
                    }
                }
            }
            Err(error) => {
                // if RPC call failed to get transactions, skip the transactions and retry later
                let error_msg = format!("Failed to get transactions: {error:?}.");
                chunk
                    .iter()
                    .for_each(|s| process_solana_signature(*s, Some(&error_msg)));
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
                process_invalid_event(signature, &error);
            }
        };
    }
}

fn process_transaction_logs(
    transaction: &GetTransactionResponse,
) -> Result<ReceivedSolEvent, String> {
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
            let deposit: ReceivedSolEvent =
                ReceivedSolEvent::from((signature.as_str(), solana_address.as_str(), base64_data));

            return Ok(deposit);
        } else {
            return Err(String::from(
                "Deposit transaction found. Invalid deposit data.",
            ));
        }
    } else {
        return Err(String::from("Non-Deposit transaction found."));
    }
}

pub async fn mint_cksol() {
    use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
    use icrc_ledger_types::icrc1::{account::Account, transfer::TransferArg};

    let _guard = match TimerGuard::new(TaskType::MintCkSol) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let (ledger_canister_id, events) = read_state(|s| (s.ledger_id, s.accepted_events.clone()));
    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id,
    };

    for (_, mut event) in events {
        match client
            .transfer(TransferArg {
                from_subaccount: None,
                to: Account {
                    owner: event.to_icp_address,
                    subaccount: None,
                },
                fee: None,
                created_at_time: None,
                memo: Some(event.clone().into()),
                amount: candid::Nat::from(event.amount),
            })
            .await
        {
            Ok(Ok(block_index)) => {
                let block_index = block_index.0.to_u64().expect("nat does not fit into u64");
                event.update_mint_block_index(block_index);
                process_minted_event(&event);
            }
            Ok(Err(err)) => {
                let error_msg = &format!("Failed to mint ckSol: {event:?} {err}");
                process_accepted_event(&event, Some(error_msg));
            }
            Err(err) => {
                let error_msg = &format!(
                    "Failed to send a message to the ledger ({ledger_canister_id}): {err:?}"
                );
                process_accepted_event(&event, Some(error_msg));
            }
        };
    }
}

/// Process events
fn process_minted_event(event: &ReceivedSolEvent) {
    ic_canister_log::log!(
        DEBUG,
        "Signature: {} -> Minted {} to {} in block {}",
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

fn process_accepted_event(event: &ReceivedSolEvent, error_msg: Option<&str>) {
    if let Some(error_msg) = error_msg {
        ic_canister_log::log!(DEBUG, "Signature: {} -> {}", event.sol_sig, error_msg);
    } else {
        ic_canister_log::log!(
            DEBUG,
            "Signature: {} -> Deposit transaction found.",
            event.sol_sig
        );
    }

    mutate_state(|s| {
        process_event(
            s,
            EventType::AcceptedEvent {
                event_source: event.clone(),
                fail_reason: error_msg.map(|s| s.to_string()),
            },
        )
    });
}

fn process_invalid_event(signature: &SolanaSignature, error_msg: &str) {
    ic_canister_log::log!(DEBUG, "Signature: {} -> {}.", signature.sol_sig, error_msg);
    mutate_state(|s| {
        process_event(
            s,
            EventType::InvalidEvent {
                signature: signature.clone(),
                fail_reason: error_msg.to_string(),
            },
        );
    });
}

fn process_solana_signature(signature: &SolanaSignature, error_msg: Option<&str>) {
    if let Some(error_msg) = error_msg {
        ic_canister_log::log!(DEBUG, "{}", error_msg);
    } else {
        ic_canister_log::log!(
            DEBUG,
            "Signature: {} -> Transaction found.",
            signature.sol_sig
        );
    }

    mutate_state(|s| {
        process_event(
            s,
            EventType::SolanaSignature {
                signature: signature.clone(),
                fail_reason: error_msg.map(|s| s.to_string()),
            },
        );
    });
}

fn process_new_solana_signature_range(newest_signature: &str, until_signature: &str) {
    ic_canister_log::log!(DEBUG, "New signature found: {newest_signature:?}.",);

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
    error: &str,
) {
    let error_msg = format!("Failed to get signatures for address: before: {before_signature}, until: {until_signature}. Error: {error:?}.");
    ic_canister_log::log!(DEBUG, "{}", error_msg);

    mutate_state(|s| {
        process_event(
            s,
            EventType::RetrySolanaSignatureRange {
                range: range.clone(),
                failed_sub_range: Some(SolanaSignatureRange::new(
                    before_signature.to_string(),
                    until_signature.to_string(),
                )),
                fail_reason: error_msg.to_string(),
            },
        )
    });
}

fn remove_solana_signature_range(range: &SolanaSignatureRange) {
    ic_canister_log::log!(
        DEBUG,
        "Range completed: before: {}, until: {}.",
        range.before_sol_sig,
        range.until_sol_sig,
    );

    mutate_state(|s| {
        process_event(s, EventType::RemoveSolanaSignatureRange(range.clone()));
    });
}
