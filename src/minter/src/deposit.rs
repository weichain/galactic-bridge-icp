use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, TaskType};

#[derive(Debug)]
pub struct DepositEvent {
    pub address_icp: String,
    pub amount: u64,
}

impl From<&str> for DepositEvent {
    fn from(s: &str) -> Self {
        use base64::prelude::*;
        let bytes = BASE64_STANDARD.decode(s).unwrap();

        let amount_bytes = &bytes[bytes.len() - 8..];
        let mut amount: u64 = 0;
        for i in 0..8 {
            amount |= (amount_bytes[i] as u64) << (i * 8);
        }

        let address_bytes = &bytes[12..bytes.len() - 8];
        let address_icp = String::from_utf8_lossy(&address_bytes);

        DepositEvent {
            address_icp: address_icp.to_string(),
            amount,
        }
    }
}

pub async fn scrap_solana_contract() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    const LIMIT: u64 = 10;

    let rpc_client = read_state(SolRpcClient::from_state);
    let mut signatures_result: Vec<SignatureResponse> = Vec::new();
    let mut before: Option<String> = None;
    let until = read_state(|s| s.get_last_scraped_transaction());

    loop {
        ic_canister_log::log!(
            DEBUG,
            "Getting signatures for address: limit: {}, before: {:?}, until: {}",
            LIMIT,
            before,
            until
        );

        let chunk = rpc_client
            .get_signatures_for_address(LIMIT, before.clone(), until.clone())
            .await;

        match chunk {
            Ok(signatures) => {
                if signatures.is_empty() {
                    ic_canister_log::log!(
                        DEBUG,
                        "No signatures for address available: limit: {}, before: {:?}, until: {}",
                        LIMIT,
                        before,
                        until
                    );
                    break;
                }

                let last_signature = signatures.last().unwrap();
                before = Some(last_signature.signature.clone());
                signatures_result.extend(signatures);
            }
            Err(error) => {
                ic_canister_log::log!(DEBUG, "Failed to get signatures for address: {:?}", error);
                return;
            }
        };
    }

    // update last scraped transaction
    mutate_state(|s| {
        s.last_scraped_transaction = signatures_result.first().map(|r| r.signature.clone())
    });

    let signatures: Vec<String> = signatures_result
        .iter()
        .map(|response| response.signature.to_string())
        .collect();
    let mut transactions_result: Vec<GetTransactionResponse> = Vec::new();

    for chunk in signatures.chunks(LIMIT as usize) {
        ic_canister_log::log!(DEBUG, "Getting transactions: {:?}", chunk);

        let transactions_chunk = rpc_client.get_transactions(chunk.to_vec()).await;

        match transactions_chunk {
            Ok(transactions) => {
                transactions_result.extend(transactions);
            }
            Err(error) => {
                ic_canister_log::log!(DEBUG, "Failed to get transactions: {:?}", error);
                return;
            }
        };
    }

    // transform to deposit event
    let deposit_msg: String = String::from("Program log: Instruction: Deposit");
    let success_msg: String = format!(
        "Program {} success",
        &read_state(|s| s.solana_contract_address.clone())
    );
    let program_data_msg: String = String::from("Program data: ");

    for transaction in transactions_result {
        let msgs = &transaction.meta.logMessages;

        if msgs.contains(&deposit_msg)
            && msgs.contains(&success_msg)
            && msgs.iter().any(|s| s.starts_with(&program_data_msg))
        {
            if let Some(program_data) = msgs.iter().find(|s| s.starts_with(&program_data_msg)) {
                // Extract the data after "Program data: "
                let base64_data = program_data.trim_start_matches(&program_data_msg);

                let deposit_event = DepositEvent::from(base64_data);
                ic_canister_log::log!(DEBUG, "Deposit instruction found: {:?}", deposit_event);
            } else {
                ic_canister_log::log!(DEBUG, "Deposit instruction found. No program data found");
            }
        } else {
            ic_canister_log::log!(DEBUG, "Non Deposit instruction. Skipping...");
        }
    }
}
