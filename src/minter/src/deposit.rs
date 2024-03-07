use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::{GetTransactionResponse, SignatureResponse};
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, TaskType};

use ic_canister_log::log;

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

    use serde::{Deserialize, Serialize};
    #[derive(Debug, Deserialize, Serialize)]
    pub struct DepositEvent {
        pub address_icp: String,
        pub amount: u64,
    }
    impl From<&str> for DepositEvent {
        fn from(data: &str) -> Self {
            // Assuming data is a base64 encoded string
            let decoded_data = base64::decode(data).expect("Failed to decode base64 data");

            // Assuming the decoded data is in a specific format, e.g., address_icp and amount separated by some delimiter
            let decoded_string =
                String::from_utf8(decoded_data).expect("Failed to convert decoded data to string");
            let parts: Vec<&str> = decoded_string.split(',').collect();

            // Assuming the first part is the address_icp and the second part is the amount
            let address_icp = parts.get(0).unwrap_or(&"").to_string();
            let amount = parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);

            DepositEvent {
                address_icp,
                amount,
            }
        }
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
                    let base64_data = &data[index + "Program data: ".len()..];
                    ic_cdk::println!("Base64 Data: {}", base64_data);

                    let de = DepositEvent::from(base64_data);
                    ic_cdk::println!("deposit event: {:?}", de);
                }
            } else {
                ("Deposit instruction found. No program data found");
            }
        } else {
            ic_cdk::println!("Non Deposit instruction. Skipping...");
        }
    });
}
