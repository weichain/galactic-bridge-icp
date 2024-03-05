use crate::guard::TimerGuard;
use crate::logs::DEBUG;
use crate::state::{read_state, TaskType};

use ic_canister_log::log;

pub async fn scrap_eth_logs() {
    let _guard = match TimerGuard::new(TaskType::ScrapSolLogs) {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let contract_address = read_state(|s| s.solana_contract_address.clone());

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
