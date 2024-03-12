use crate::events::{DepositEvent, InvalidTransaction, SkippedSignatureRange, SkippedTransaction};
use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, State, TaskType};

use std::collections::HashMap;

const GET_SIGNATURES_BY_ADDRESS_LIMIT: u64 = 10;
const GET_TRANSACTIONS_LIMIT: u64 = 5;

pub async fn scrap_solana_contract() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);

    let (signatures_result, skipped_signature_range) =
        scrap_signatures_with_limit(&rpc_client, None).await;

    let signatures: Vec<String> = signatures_result
        .iter()
        .map(|response| response.signature.to_string())
        .collect();

    let (transactions_result, skipped_transactions) =
        scrap_transactions_with_limit(&rpc_client, signatures, None).await;

    let (deposit_events, invalid_transactions) = parse_log_messages(transactions_result).await;

    // update last scraped transaction
    mutate_state(|s: &mut State| {
        s.last_scraped_transaction = signatures_result.first().map(|r| r.signature.clone());

        skipped_signature_range.map(|range| {
            s.record_skipped_signature_range(range);
        });

        skipped_transactions.iter().for_each(|tx| {
            s.record_skipped_transaction(tx.clone());
        });

        invalid_transactions.iter().for_each(|tx| {
            s.record_invalid_transaction(tx.clone());
        });

        deposit_events.iter().for_each(|(k, v)| {
            s.events_to_mint.insert(k.clone(), v.clone());
        });
    });
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
async fn scrap_signatures_with_limit(
    rpc_client: &SolRpcClient,
    limit: Option<u64>,
) -> (Vec<SignatureResponse>, Option<SkippedSignatureRange>) {
    let limit = limit.unwrap_or(GET_SIGNATURES_BY_ADDRESS_LIMIT);
    let mut signatures_result: Vec<SignatureResponse> = Vec::new();
    let mut before_signature: Option<String> = None;
    let until_signature = read_state(|s| s.get_last_scraped_transaction());

    loop {
        ic_canister_log::log!(
            DEBUG,
            "Getting signatures for address: limit: {limit}, before: {before_signature:?}, until: {until_signature}.",
        );

        // get signatures for chunk
        let chunk = rpc_client
            .get_signatures_for_address(limit, before_signature.clone(), until_signature.clone())
            .await;

        match chunk {
            Ok(signatures) => {
                // if no signatures are available, we are done
                if signatures.is_empty() {
                    ic_canister_log::log!(
                        DEBUG,
                        "No signatures for address available: limit: {limit}, before: {before_signature:?}, until: {until_signature}.",
                    );

                    return (signatures_result, None);
                }

                // if signatures are available, we continue with the next chunk
                // store the last signature to use it as before for the next chunk
                let last_signature = signatures.last().unwrap();
                before_signature = Some(last_signature.signature.clone());
                signatures_result.extend(signatures);
            }
            Err(error) => {
                ic_canister_log::log!(DEBUG, "Failed to get signatures for address: {:?}.", error);

                // if rpc request fails to get signatures, cannot continue, skip the range and retry later
                if let Some(before) = before_signature {
                    // in case "before_signature" is not None return the skipped range
                    return (
                        signatures_result,
                        Some(SkippedSignatureRange::new(
                            before,
                            until_signature,
                            format!("{:?}", error),
                        )),
                    );
                }

                // in case rpc request fails on first run ("before_signature" is None), skip the whole range
                return (signatures_result, None);
            }
        };
    }
}

async fn scrap_transactions_with_limit(
    rpc_client: &SolRpcClient,
    signatures: Vec<String>,
    limit: Option<u64>,
) -> (Vec<GetTransactionResponse>, Vec<SkippedTransaction>) {
    let limit = limit.unwrap_or(GET_TRANSACTIONS_LIMIT);
    let mut transactions_result: Vec<GetTransactionResponse> = Vec::new();
    let mut skipped_transactions: Vec<SkippedTransaction> = Vec::new();

    for chunk in signatures.chunks(limit as usize) {
        ic_canister_log::log!(DEBUG, "Getting transactions: {chunk:?}.");

        let transactions_chunk = rpc_client.get_transactions(chunk.to_vec()).await;

        match transactions_chunk {
            Ok(txs) => {
                txs.iter().for_each(|(k, v)| {
                    // if tx call failed
                    if let Err(error) = v {
                        ic_canister_log::log!(DEBUG, "Signature: {k} -> Failed with {error:?}.",);

                        skipped_transactions.push(SkippedTransaction::new(
                            k.to_string(),
                            format!("{error:?}",),
                        ));
                    } else {
                        // if tx call returned a proper object
                        if let Some(tx) = v.as_ref().unwrap() {
                            transactions_result.push(tx.clone());
                        } else {
                            ic_canister_log::log!(
                                DEBUG,
                                "Signature: {k} -> Transaction not found.",
                            );

                            // if tx call returned NONE
                            skipped_transactions.push(SkippedTransaction::new(
                                k.to_string(),
                                "Transaction not found".to_string(),
                            ));
                        }
                    }
                })
                // save transactions
            }
            Err(error) => {
                // if RPC call failed to get transactions, skip the transactions and retry later
                ic_canister_log::log!(DEBUG, "Failed to get transactions: {error:?}.");

                skipped_transactions.extend(chunk.iter().map(|signature| {
                    SkippedTransaction::new(signature.clone(), format!("{error:?}"))
                }));
            }
        };
    }

    return (transactions_result, skipped_transactions);
}

async fn parse_log_messages(
    transactions: Vec<GetTransactionResponse>,
) -> (HashMap<String, DepositEvent>, Vec<InvalidTransaction>) {
    // transform to deposit event
    let deposit_msg: String = String::from("Program log: Instruction: Deposit");
    let success_msg: String = format!(
        "Program {} success",
        &read_state(|s| s.solana_contract_address.clone())
    );
    let program_data_msg: String = String::from("Program data: ");

    // filter transactions with deposit event
    let mut deposits = HashMap::<String, DepositEvent>::new();
    // invalid transactions (non deposit)
    let mut invalid_transactions = Vec::<InvalidTransaction>::new();

    for transaction in transactions {
        let msgs = &transaction.meta.logMessages;
        let sig = transaction.transaction.signatures[0].to_string();

        if msgs.contains(&deposit_msg)
            && msgs.contains(&success_msg)
            && msgs.iter().any(|s| s.starts_with(&program_data_msg))
        {
            if let Some(program_data) = msgs.iter().find(|s| s.starts_with(&program_data_msg)) {
                // Extract the data after "Program data: "
                let base64_data = program_data.trim_start_matches(&program_data_msg);
                let deposit_event = DepositEvent::from(base64_data);

                ic_canister_log::log!(
                    DEBUG,
                    "Signature: {sig} -> Deposit transaction found: {deposit_event:?}.",
                );

                deposits.insert(sig, deposit_event);
            } else {
                let error_msg = "Deposit transaction found. Invalid deposit data.".to_string();

                ic_canister_log::log!(DEBUG, "Signature: {sig} -> {error_msg}.");

                invalid_transactions.push(InvalidTransaction::new(sig, error_msg))
            }
        } else {
            let error_msg = "Non Deposit transaction found.".to_string();

            ic_canister_log::log!(DEBUG, "Signature: {sig} -> {error_msg}.");

            invalid_transactions.push(InvalidTransaction::new(sig, error_msg))
        }
    }

    return (deposits, invalid_transactions);
}
