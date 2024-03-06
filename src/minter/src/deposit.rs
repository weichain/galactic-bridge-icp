use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::sol_rpc_client::responses::SignatureResponse;
use crate::sol_rpc_client::SolRpcClient;
use crate::state::{read_state, TaskType};

use ic_canister_log::log;

pub async fn scrap_sol() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let rpc_client = read_state(SolRpcClient::from_state);

    const LIMIT: u64 = 10;

    let mut before: Option<String> = None;
    let until: String =
        "2qaGQLHQvfnjNonadNk3FQ17tSJQ7rCqSXUckaa7qhJQWGmHYptxW7V98YxFK1sE8NWz8G3nU5b3BzWbJur3ZhU4"
            .to_string();

    let mut result: Vec<SignatureResponse> = vec![];

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

    ic_cdk::println!("result: {:?} {:?}", result, result.len());

    // TODO: implement this
    // let last_block_number = match update_last_observed_block_number().await {
    //     Some(block_number) => block_number,
    //     None => {
    //         log!(
    //             DEBUG,
    //             "[scrap_eth_logs]: skipping scrapping ETH logs: no last observed block number"
    //         );
    //         return;
    //     }
    // };
    // let mut last_scraped_block_number = read_state(|s| s.last_scraped_block_number);

    // while last_scraped_block_number < last_block_number {
    //     let next_block_to_query = last_scraped_block_number
    //         .checked_increment()
    //         .unwrap_or(BlockNumber::MAX);
    //     last_scraped_block_number = match scrap_eth_logs_range_inclusive(
    //         contract_address,
    //         next_block_to_query,
    //         last_block_number,
    //     )
    //     .await
    //     {
    //         Some(last_scraped_block_number) => last_scraped_block_number,
    //         None => {
    //             return;
    //         }
    //     };
    // }
}
