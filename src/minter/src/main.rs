use minter::{
    constants::{
        GET_LATEST_SOLANA_SIGNATURE, MINT_GSOL, SCRAPPING_SOLANA_SIGNATURES,
        SCRAPPING_SOLANA_SIGNATURE_RANGES,
    },
    deposit::{get_latest_signature, mint_gsol, scrap_signature_range, scrap_signatures},
    lifecycle::{post_upgrade as lifecycle_post_upgrade, MinterArg},
    logs::INFO,
    // sol_rpc_client::types::Error,
    state::{event::EventType, lazy_call_ecdsa_public_key, read_state, State, STATE},
    storage,
    withdraw::{
        get_coupon as get_or_regen_coupon, get_withdraw_info as get_user_withdraw_info,
        withdraw_gsol, Coupon, CouponError, UserWithdrawInfo, WithdrawError,
    },
};

use candid::candid_method;
use ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use num_bigint::BigUint;
use num_traits::cast::ToPrimitive;
use std::time::Duration;

/// Sets up timers for various tasks, such as fetching latest signatures and scraping logs.
fn setup_timers() {
    // Set timer to fetch ECDSA public key immediately after install.
    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(async {
            let _ = lazy_call_ecdsa_public_key().await;
        });
    });

    // Set timers for scraping logs and other operations with specified intervals.
    // These timers are started immediately after installation.
    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(async {
            get_latest_signature().await;
            scrap_signature_range().await;
            scrap_signatures().await;
            mint_gsol().await;
        });
    });

    // Set intervals for periodic tasks.
    ic_cdk_timers::set_timer_interval(GET_LATEST_SOLANA_SIGNATURE, || {
        ic_cdk::spawn(async {
            get_latest_signature().await;
        });
    });

    ic_cdk_timers::set_timer_interval(SCRAPPING_SOLANA_SIGNATURE_RANGES, || {
        ic_cdk::spawn(async {
            scrap_signature_range().await;
        });
    });

    ic_cdk_timers::set_timer_interval(SCRAPPING_SOLANA_SIGNATURES, || {
        ic_cdk::spawn(async {
            scrap_signatures().await;
        });
    });

    ic_cdk_timers::set_timer_interval(MINT_GSOL, || {
        ic_cdk::spawn(async {
            mint_gsol().await;
        });
    });
}

/// Initializes the Minter canister with the given arguments.
///
/// # Arguments
///
/// * `args` - Initialization arguments for the Minter canister.
#[candid_method(init)]
#[init]
pub fn init(args: MinterArg) {
    // Match on the initialization arguments.
    match args {
        // If the argument is an initialization argument, initialize the state.
        MinterArg::Init(init_arg) => {
            ic_canister_log::log!(INFO, "\ninitialized minter with arg:\n{init_arg:?}");
            STATE.with(|cell| {
                storage::record_event(EventType::Init(init_arg.clone()));
                *cell.borrow_mut() =
                    Some(State::try_from(init_arg).expect("failed to initialize minter"))
            });
        }
        // If the argument is an upgrade argument, trap with an error message.
        MinterArg::Upgrade(_) => {
            ic_cdk::trap("cannot init canister state with upgrade args");
        }
    }

    // Setup timers for periodic tasks.
    setup_timers();
}

/// Performs actions before upgrading the canister state.
#[pre_upgrade]
fn pre_upgrade() {
    read_state(|s| {
        storage::record_event(EventType::LastKnownSolanaSignature(
            s.get_solana_last_known_signature(),
        ));
        storage::record_event(EventType::LastDepositIdCounter(s.deposit_id_counter));
        storage::record_event(EventType::LastBurnIdCounter(s.burn_id_counter));
    });
}

/// Performs actions after upgrading the canister state.
#[post_upgrade]
fn post_upgrade(minter_arg: Option<MinterArg>) {
    match minter_arg {
        Some(MinterArg::Init(_)) => {
            ic_cdk::trap("cannot upgrade canister state with init args");
        }
        Some(MinterArg::Upgrade(upgrade_args)) => lifecycle_post_upgrade(Some(upgrade_args)),
        None => lifecycle_post_upgrade(None),
    }

    // Setup timers for periodic tasks after upgrade.
    setup_timers();
}

/// Returns the compressed and uncompressed public keys.
#[update]
pub async fn get_address() -> (String, String) {
    read_state(|s| (s.compressed_public_key(), s.uncompressed_public_key()))
}

/// Withdraws GSOL tokens to the specified Solana address.
///
/// # Arguments
///
/// * `solana_address` - The Solana address to withdraw GSOL tokens to.
/// * `withdraw_amount` - The amount of GSOL tokens to withdraw.
#[update]
async fn withdraw(
    solana_address: String,
    withdraw_amount: candid::Nat,
) -> Result<Coupon, WithdrawError> {
    let caller = validate_caller_not_anonymous();
    is_over_limit(&withdraw_amount.0);

    withdraw_gsol(caller, solana_address, withdraw_amount).await
}

/// Gets coupon or tries to regenerate coupon if it is not found.
///
/// # Arguments
///
/// * `burn_id` - Burn id of the coupon.
#[update]
async fn get_coupon(burn_id: u64) -> Result<Coupon, WithdrawError> {
    let caller = validate_caller_not_anonymous();

    get_or_regen_coupon(caller, burn_id).await
}

/// Returns ledger id.
#[query]
async fn get_withdraw_info() -> UserWithdrawInfo {
    let caller = validate_caller_not_anonymous();

    get_user_withdraw_info(caller).await
}

/// Returns ledger id.
#[query]
async fn get_ledger_id() -> String {
    read_state(|s| s.ledger_id.clone().to_string())
}

/// Verification method that validates coupon.
#[query]
async fn verify(coupon: Coupon) -> Result<bool, CouponError> {
    coupon.verify()
}

/// Cleans up the HTTP response headers to make them deterministic.
///
/// # Arguments
///
/// * `args` - Transformation arguments containing the HTTP response.
#[query(hidden = true)]
fn cleanup_response(mut args: TransformArgs) -> HttpResponse {
    // The response header contain non-deterministic fields that make it impossible to reach consensus!
    // Errors seem deterministic and do not contain data that can break consensus.

    // Clear non-deterministic fields from the response headers.
    args.response.headers.clear();

    args.response
}

/// Returns the current state of the Minter canister.
#[query]
fn get_state() -> String {
    is_controller();

    read_state(|s| {
        ic_canister_log::log!(INFO, "state: {:?}", s);
        s.to_string()
    })
}

/// Returns the storage events recorded in the Minter canister.
#[query]
fn get_storage() -> String {
    is_controller();

    use std::fmt::Write;

    let events = minter::storage::get_storage_events();
    let mut result = String::new();
    for event in events {
        write!(
            &mut result,
            "Event(timestamp: {}, payload: {:?})\n",
            event.timestamp, event.payload
        )
        .unwrap();
    }
    result
}

/// Returns active tasks in the Minter canister.
#[query]
fn get_active_tasks() {
    is_controller();

    read_state(|s| ic_canister_log::log!(INFO, "active_tasks: {:?}", s.active_tasks));
}

fn main() {}
ic_cdk_macros::export_candid!();

fn validate_caller_not_anonymous() -> candid::Principal {
    let principal = ic_cdk::caller();
    if principal == candid::Principal::anonymous() {
        ic_cdk::trap("anonymous principal is not allowed");
    }
    principal
}

fn is_controller() -> candid::Principal {
    let principal = ic_cdk::caller();
    if !ic_cdk::api::is_controller(&principal) {
        ic_cdk::trap("only controller can call this method");
    }

    principal
}

fn is_over_limit(withdraw_amount: &BigUint) {
    let minimum = read_state(|s| s.minimum_withdrawal_amount.clone());

    match minimum.cmp(&withdraw_amount) {
        std::cmp::Ordering::Greater => {
            ic_cdk::trap("withdraw amount is less than minimum withdrawal amount");
        }
        _ => {}
    }
}
