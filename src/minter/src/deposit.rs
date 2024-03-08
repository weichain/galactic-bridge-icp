use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, TaskType};

use ic_canister_log::log;

#[derive(Debug)]
pub struct DepositEvent {
    pub address_icp: String,
    pub amount: u64,
}

impl DepositEvent {
    fn from_string(s: &str) -> Self {
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
        let chunk = rpc_client
            .get_signatures_for_address(LIMIT, before.clone(), until.clone())
            .await;

        match chunk {
            None => {
                log!(DEBUG, "Failed to get signatures for address");
                return;
            }
            Some(signatures) => {
                if signatures.is_empty() {
                    break;
                }

                let last_signature = signatures.last().unwrap();
                before = Some(last_signature.signature.clone());
                signatures_result.extend(signatures);
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
        let transactions_chunk = rpc_client.get_transactions(chunk.to_vec()).await;

        match transactions_chunk {
            None => {
                log!(DEBUG, "Failed to get signatures for address");
                return;
            }
            Some(transactions) => {
                transactions_result.extend(transactions);
            }
        };
    }

    transactions_result.iter().for_each(|transaction| {
        // get log messages
        let msgs = &transaction.meta.logMessages;

        // check if one of the log messages contains "Instruction: Deposit"
        if let Some(_) = msgs
            .iter()
            .find(|&instr| instr.contains("Instruction: Deposit"))
        {
            // check if one of the log messages contains "Program data: "
            // TODO:
            if let Some(data) = msgs.iter().find(|&instr| instr.contains("Program data: ")) {
                // get program data and parse it
                if let Some(index) = data.find("Program data: ") {
                    let base64_data = &data[index + "Program data: ".len()..].trim();

                    let deposit_event = DepositEvent::from_string(base64_data);
                    ic_cdk::println!("Deposit instruction found: {:?}", deposit_event);
                }
            } else {
                ic_cdk::println!("Deposit instruction found. No program data found");
            }
        } else {
            ic_cdk::println!("Non Deposit instruction. Skipping...");
        }
    });
}
