use crate::events::{
    Deposit, DepositEvent, InvalidSolTransaction, SkippedSolSignatureRange, SkippedSolTransaction,
};
use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::audit::process_event;
use crate::state::event::EventType;
use crate::state::{mutate_state, read_state, TaskType};

const GET_SIGNATURES_BY_ADDRESS_LIMIT: u64 = 10;
const GET_TRANSACTIONS_LIMIT: u64 = 5;

pub async fn scrap_solana_logs() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);
    let until_signature = read_state(|s| s.get_last_scraped_transaction());

    let signatures =
        scrap_signature_range_with_limit(&rpc_client, None, &until_signature, None).await;

    let transactions =
        scrap_transactions_with_limit(&rpc_client, signatures.iter().collect(), None).await;

    transactions.last().map(|tx| {
        mutate_state(|s| {
            process_event(
                s,
                EventType::SyncedToSignature {
                    signature: tx.transaction.signatures[0].to_string(),
                },
            )
        })
    });

    parse_log_messages(transactions.iter().collect()).await;
}

// Method relies on the getSignaturesForAddress RPC call to get the signatures for the address:
// https://solana.com/docs/rpc/http/getsignaturesforaddress
//
// The method is called with a limit, which is the maximum number of signatures to be returned on a single call:
// On first call:
// before_signature is unknown -> If not provided the search starts from the top of the highest max confirmed block
// until_signature is the last scraped transaction
//
// On subsequent calls:
// before_signature is the last signature from the previous call
// until_signature is the last scraped transaction
//
// If first call fails range is not marked as skipped (it will be covered on next calls).
// But in case subsequent call fails (it means gap appears in the data) scrap process stops and the failed
// range is marked as skipped (for later retry)!
async fn scrap_signature_range_with_limit(
    rpc_client: &SolRpcClient,
    mut before_signature: Option<String>,
    until_signature: &String,
    limit: Option<u64>,
) -> Vec<String> {
    let limit = limit.unwrap_or(GET_SIGNATURES_BY_ADDRESS_LIMIT);
    let mut signatures_result: Vec<SignatureResponse> = Vec::new();

    fn transform_signature_response(response: &Vec<SignatureResponse>) -> Vec<String> {
        response.iter().map(|r| r.signature.to_string()).collect()
    }

    loop {
        ic_canister_log::log!(
            DEBUG,
            "Getting signatures for address: limit: {limit}, before: {before_signature:?}, until: {until_signature}.",
        );

        // get signatures for chunk
        match rpc_client
            .get_signatures_for_address(limit, before_signature.as_ref(), until_signature)
            .await
        {
            Ok(signatures) => {
                // if no signatures are available, we are done
                if signatures.is_empty() {
                    ic_canister_log::log!(
                        DEBUG,
                        "No signatures for address available: limit: {limit}, before: {before_signature:?}, until: {until_signature}.",
                    );

                    return transform_signature_response(&signatures_result);
                }

                // if signatures are available, we continue with the next chunk
                // store the last signature to use it as before for the next chunk
                let last_signature = signatures.last().unwrap();
                before_signature = Some(last_signature.signature.to_string());
                signatures_result.extend(signatures);
            }
            Err(error) => {
                ic_canister_log::log!(DEBUG, "Failed to get signatures for address: {:?}.", error);

                // if rpc request fails to get signatures, cannot continue, skip the range and retry later
                if let Some(before) = before_signature {
                    // in case "before_signature" is not None return the skipped range

                    mutate_state(|s| {
                        process_event(
                            s,
                            EventType::SkippedSolSignatureRange {
                                range: SkippedSolSignatureRange::new(
                                    before.to_string(),
                                    until_signature.to_string(),
                                ),
                                reason: format!("{:?}", error),
                            },
                        )
                    });
                }

                // in case rpc request fails on first run ("before_signature" is None), skip the whole range
                // no need to mark it again, it will be covered on next calls
                return transform_signature_response(&signatures_result);
            }
        };
    }
}

async fn scrap_transactions_with_limit(
    rpc_client: &SolRpcClient,
    signatures: Vec<&String>,
    limit: Option<u64>,
) -> Vec<GetTransactionResponse> {
    let limit = limit.unwrap_or(GET_TRANSACTIONS_LIMIT);
    let mut transactions_result: Vec<GetTransactionResponse> = Vec::new();

    for chunk in signatures.chunks(limit as usize) {
        match rpc_client.get_transactions(chunk.to_vec()).await {
            Ok(txs) => {
                txs.iter().for_each(|(k, v)| {
                    // if tx call failed
                    if let Err(error) = v {
                        ic_canister_log::log!(DEBUG, "Signature: {k} -> Failed with {error:?}.",);

                        // skip the transaction and retry later
                        mutate_state(|s| {
                            process_event(
                                s,
                                EventType::SkippedSolTransaction {
                                    sol_tx: SkippedSolTransaction::new(k.to_string()),
                                    reason: format!("{:?}", error),
                                },
                            )
                        });
                    } else {
                        // if tx call returned a proper object
                        if let Some(tx) = v.as_ref().unwrap() {
                            transactions_result.push(tx.clone());
                        } else {
                            // if tx call returned None
                            ic_canister_log::log!(
                                DEBUG,
                                "Signature: {k} -> Transaction not found."
                            );

                            mutate_state(|s| {
                                process_event(
                                    s,
                                    EventType::SkippedSolTransaction {
                                        sol_tx: SkippedSolTransaction::new(k.to_string()),
                                        reason: "Transaction not found".to_string(),
                                    },
                                )
                            });
                        }
                    }
                })
                // save transactions
            }
            Err(error) => {
                // if RPC call failed to get transactions, skip the transactions and retry later
                ic_canister_log::log!(DEBUG, "Failed to get transactions: {error:?}.");

                mutate_state(|s| {
                    chunk.iter().for_each(|signature| {
                        process_event(
                            s,
                            EventType::SkippedSolTransaction {
                                sol_tx: SkippedSolTransaction::new(signature.to_string()),
                                reason: format!("{error:?}"),
                            },
                        )
                    });
                });
            }
        };
    }

    return transactions_result;
}

async fn parse_log_messages(transactions: Vec<&GetTransactionResponse>) {
    // transform to deposit event
    let deposit_msg = &String::from("Program log: Instruction: Deposit");
    let success_msg = &format!(
        "Program {} success",
        &read_state(|s| s.solana_contract_address.clone())
    );
    let program_data_msg = &String::from("Program data: ");

    for transaction in transactions {
        let msgs = &transaction.meta.logMessages;
        let signature: String = transaction.transaction.signatures[0].to_string();

        if msgs.contains(deposit_msg)
            && msgs.contains(success_msg)
            && msgs.iter().any(|s| s.starts_with(program_data_msg))
        {
            if let Some(program_data) = msgs.iter().find(|s| s.starts_with(program_data_msg)) {
                // Extract the data after "Program data: "
                let base64_data = program_data.trim_start_matches(program_data_msg);
                let deposit = Deposit::from(base64_data);

                ic_canister_log::log!(
                    DEBUG,
                    "Signature: {signature} -> Deposit transaction found: {deposit:?}.",
                );

                mutate_state(|s| {
                    process_event(
                        s,
                        EventType::AcceptedDeposit {
                            deposit: deposit,
                            sol_sig: signature,
                        },
                    )
                });
            } else {
                let error_msg = "Deposit transaction found. Invalid deposit data.".to_string();

                ic_canister_log::log!(DEBUG, "Signature: {signature} -> {error_msg}.");

                mutate_state(|s| {
                    process_event(
                        s,
                        EventType::InvalidDeposit {
                            sol_tx: InvalidSolTransaction::new(signature),
                            reason: error_msg,
                        },
                    )
                });
            }
        } else {
            let error_msg = "Non Deposit transaction found.".to_string();

            ic_canister_log::log!(DEBUG, "Signature: {signature} -> {error_msg}.");

            mutate_state(|s| {
                process_event(
                    s,
                    EventType::InvalidDeposit {
                        sol_tx: InvalidSolTransaction::new(signature),
                        reason: error_msg,
                    },
                )
            });
        }
    }
}
