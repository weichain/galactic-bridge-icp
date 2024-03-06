use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::SignatureResponse;
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{mutate_state, read_state, TaskType};

use ic_canister_log::log;

pub async fn scrap_sol() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    const LIMIT: u64 = 10;

    let rpc_client = read_state(SolRpcClient::from_state);
    let mut result: Vec<SignatureResponse> = Vec::new();
    let mut before: Option<String> = None;
    let until = read_state(|s| s.get_last_scraped_transaction());

    loop {
        let chunk = rpc_client
            .get_signatures_for_address(LIMIT, before.clone(), until.clone())
            .await;

        ic_cdk::println!("result: {:?}", chunk);

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
                result.extend(signatures);
            }
        };
    }

    // update last scraped transaction
    mutate_state(|s| s.last_scraped_transaction = result.first().map(|r| r.signature.clone()));

    ic_cdk::println!("result: {:?} {:?}", result, result.len());
}
