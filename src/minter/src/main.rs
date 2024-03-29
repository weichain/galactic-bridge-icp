use minter::{
    constants::{
        GET_LATEST_SOLANA_SIGNATURE, MINT_CKSOL, SCRAPPING_SOLANA_SIGNATURES,
        SCRAPPING_SOLANA_SIGNATURE_RANGES,
    },
    deposit::{get_latest_signature, mint_cksol, scrap_signature_range, scrap_signatures},
    lifecycle::{post_upgrade as lifecycle_post_upgrade, MinterArg},
    logs::INFO,
    state::{event::EventType, lazy_call_ecdsa_public_key, read_state, State, STATE},
    storage,
    withdraw::{withdraw_cksol, Coupon, CouponError, WithdrawError},
};

use candid::candid_method;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use num_traits::cast::ToPrimitive;
use std::time::Duration;

fn setup_timers() {
    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(async {
            let _ = lazy_call_ecdsa_public_key().await;
        });
    });

    // Start scraping logs immediately after the install, then repeat each operation with the interval.
    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(async {
            get_latest_signature().await;
            scrap_signature_range().await;
            scrap_signatures().await;
            mint_cksol().await;
        });
    });

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

    ic_cdk_timers::set_timer_interval(MINT_CKSOL, || {
        ic_cdk::spawn(async {
            mint_cksol().await;
        });
    });
}

#[candid_method(init)]
#[init]
pub fn init(args: MinterArg) {
    match args {
        MinterArg::Init(init_arg) => {
            ic_canister_log::log!(INFO, "\ninitialized minter with arg:\n{init_arg:?}");
            STATE.with(|cell| {
                storage::record_event(EventType::Init(init_arg.clone()));
                *cell.borrow_mut() =
                    Some(State::try_from(init_arg).expect("failed to initialize minter"))
            });
        }
        MinterArg::Upgrade(_) => {
            ic_cdk::trap("cannot init canister state with upgrade args");
        }
    }

    setup_timers();
}

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

#[post_upgrade]
fn post_upgrade(minter_arg: Option<MinterArg>) {
    match minter_arg {
        Some(MinterArg::Init(_)) => {
            ic_cdk::trap("cannot upgrade canister state with init args");
        }
        Some(MinterArg::Upgrade(upgrade_args)) => lifecycle_post_upgrade(Some(upgrade_args)),
        None => lifecycle_post_upgrade(None),
    }

    setup_timers();
}

//////////////////////////
#[update]
pub async fn get_address() -> (String, String) {
    read_state(|s| (s.compressed_public_key(), s.uncompressed_public_key()))
}

#[update]
async fn withdraw(
    solana_address: String,
    withdraw_amount: candid::Nat,
) -> Result<Coupon, WithdrawError> {
    let caller = validate_caller_not_anonymous();

    withdraw_cksol(
        caller,
        solana_address,
        withdraw_amount
            .0
            .to_u64()
            .expect("withdraw amount should fit into u64"),
    )
    .await
}

#[query]
fn get_state() {
    read_state(|s| ic_canister_log::log!(INFO, "state: {:?}", s));
}

#[query]
fn get_active_tasks() {
    read_state(|s| ic_canister_log::log!(INFO, "active_tasks: {:?}", s.active_tasks));
}

#[query]
async fn get_ledger_id() -> String {
    read_state(|s| s.ledger_id.clone().to_string())
}

#[query]
async fn verify(coupon: Coupon) -> Result<bool, CouponError> {
    coupon.verify()
}

//////////////////////////
fn main() {}
ic_cdk_macros::export_candid!();

fn validate_caller_not_anonymous() -> candid::Principal {
    let principal = ic_cdk::caller();
    if principal == candid::Principal::anonymous() {
        panic!("anonymous principal is not allowed");
    }
    principal
}
