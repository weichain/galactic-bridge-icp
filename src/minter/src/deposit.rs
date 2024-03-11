use crate::events::{DepositEvent, InvalidTransaction, SkippedSignatureRange, SkippedTransaction};
use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, TaskType};

use std::collections::HashMap;

const GET_SIGNATURES_BY_ADDRESS_LIMIT: u64 = 10;
const GET_TRANSACTIONS_LIMIT: u64 = 10;

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
    mutate_state(|s| {
        s.last_scraped_transaction = signatures_result.first().map(|r| r.signature.clone())
    });
}

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
            "Getting signatures for address: limit: {}, before: {:?}, until: {}",
            limit,
            before_signature,
            until_signature
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
                        "No signatures for address available: limit: {}, before: {:?}, until: {}",
                        limit,
                        before_signature,
                        until_signature
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
                ic_canister_log::log!(DEBUG, "Failed to get signatures for address: {:?}", error);

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
        ic_canister_log::log!(DEBUG, "Getting transactions: {:?}", chunk);

        let transactions_chunk = rpc_client.get_transactions(chunk.to_vec()).await;

        match transactions_chunk {
            Ok(txs) => {
                txs.iter().for_each(|(k, v)| {
                    if let Some(tx) = v {
                        transactions_result.push(tx.clone());
                    }

                    skipped_transactions.push(SkippedTransaction::new(
                        k.to_string(),
                        "Transaction not found".to_string(),
                    ));
                })
                // save transactions
            }
            Err(error) => {
                // if RPC call failed to get transactions, skip the transactions and retry later
                ic_canister_log::log!(DEBUG, "Failed to get transactions: {:?}", error);

                skipped_transactions.extend(chunk.iter().map(|signature| {
                    SkippedTransaction::new(signature.clone(), format!("{:?}", error))
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
                    "Signature: {} -> Deposit transaction found: {:?}",
                    sig,
                    deposit_event
                );

                deposits.insert(sig, deposit_event);
            } else {
                let error_msg = "Deposit transaction found. Invalid deposit data.".to_string();

                ic_canister_log::log!(DEBUG, "Signature: {} -> {}", sig, error_msg);

                invalid_transactions.push(InvalidTransaction::new(sig, error_msg))
            }
        } else {
            let error_msg = "Non Deposit transaction found.".to_string();

            ic_canister_log::log!(DEBUG, "Signature: {} -> {}", sig, error_msg);

            invalid_transactions.push(InvalidTransaction::new(sig, error_msg))
        }
    }

    return (deposits, invalid_transactions);
}
