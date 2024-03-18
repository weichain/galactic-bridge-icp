use minter::constants::{
    GET_LATEST_SOLANA_SIGNATURE, MINT_CKSOL, SCRAPPING_SOLANA_SIGNATURES,
    SCRAPPING_SOLANA_SIGNATURE_RANGES,
};
use minter::deposit::{get_latest_signature, mint_cksol, scrap_signature_range, scrap_signatures};
use minter::lifecycle::{post_upgrade as lifecycle_post_upgrade, MinterArg};
use minter::logs::INFO;
use minter::state::event::EventType;
use minter::state::{lazy_call_ecdsa_public_key, read_state, State, STATE};
use minter::storage;
use minter::types::{
    Coupon, ECDSAPublicKey, ECDSAPublicKeyReply, EcdsaCurve, EcdsaKeyId, SignWithECDSA,
    SignWithECDSAReply, SignatureVerificationReply,
};

use candid::{candid_method, Nat, Principal};
use ic_cdk::{api::call::call_with_payment, call};
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
use icrc_ledger_types::{icrc1::transfer::TransferArg, icrc2::transfer_from::TransferFromArgs};
use num_traits::ToPrimitive;
use serde_json;
use sha2::{Digest, Sha256};
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
        });
    });

    ic_cdk_timers::set_timer(GET_LATEST_SOLANA_SIGNATURE, || {
        ic_cdk::spawn(async {
            get_latest_signature().await;
        });
    });

    ic_cdk_timers::set_timer(SCRAPPING_SOLANA_SIGNATURE_RANGES, || {
        ic_cdk::spawn(async {
            scrap_signature_range().await;
        });
    });

    ic_cdk_timers::set_timer(SCRAPPING_SOLANA_SIGNATURES, || {
        ic_cdk::spawn(async {
            scrap_signatures().await;
        });
    });

    ic_cdk_timers::set_timer(MINT_CKSOL, || {
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
            ic_cdk::println!("init_arg: {:?}", init_arg);

            ic_canister_log::log!(INFO, "[init]: initialized minter with arg: {:?}", init_arg);
            STATE.with(|cell| {
                storage::record_event(EventType::Init(init_arg.clone()));
                *cell.borrow_mut() =
                    Some(State::try_from(init_arg).expect("BUG: failed to initialize minter"))
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

// dfx canister call minter get_state
// TODO: only for testing
#[query]
fn get_state() {
    read_state(|s| ic_cdk::println!("state: {:?}", s));
}

#[query]
async fn get_ledger_id() -> String {
    read_state(|s| s.ledger_id.clone().to_string())
}

// #[update]
// pub async fn sign() -> (String, String, String) {
//     let key_name = read_state(|s| s.ecdsa_key_name.clone());

//     let coupon = Coupon {
//         address: "0xb12B5e756A894775FC32EDdf3314Bb1B1944dC34".to_string(),
//         amount: 1_000_000_000,
//     };

//     // Serialize the coupon
//     let serialized_coupon: String = serde_json::to_string(&coupon).unwrap();

//     // Hash the serialized coupon using SHA-256
//     let mut hasher = Sha256::new();
//     hasher.update(serialized_coupon.clone());
//     let hashed_coupon = hasher.finalize();

//     // Convert the hashed coupon into a Vec<u8>
//     let hashed_coupon_bytes = hashed_coupon.to_vec();
//     let coupon_hex_string = hex::encode(&hashed_coupon_bytes);

//     // Sign the hashed coupon using ECDSA
//     let signature = sign_with_ecdsa(key_name, DERIVATION_PATH, hashed_coupon_bytes).await;
//     let signature_hex_string = hex::encode(&signature);

//     return (serialized_coupon, coupon_hex_string, signature_hex_string);
// }

#[update]
async fn burn(amount: Nat) -> Nat {
    let caller = ic_cdk::caller();

    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id: read_state(|s| s.ledger_id.clone()),
    };

    ic_cdk::println!("caller: {:?}", caller.to_string());
    ic_cdk::println!("id: {:?}", ic_cdk::id().to_string());

    let args = TransferFromArgs {
        spender_subaccount: None,
        from: caller.into(),
        to: ic_cdk::id().into(),
        amount,
        fee: None,
        created_at_time: None,
        memo: None,
    };

    let block_index: u64 = match client.transfer_from(args).await {
        Ok(Ok(block_index)) => block_index
            .0
            .to_u64()
            .expect("block index should fit into u64"),
        Ok(Err(err)) => {
            ic_cdk::println!("Failed to burn: {}", err);
            0
        }
        Err(err) => {
            ic_cdk::println!("Failed to burn: {}", err.1);
            0
        }
    };

    Nat::from(block_index)
}

#[query]
async fn verify(
    signature_hex: String,
    message: String,
    public_key_hex: String,
) -> Result<SignatureVerificationReply, String> {
    use k256::ecdsa::{signature::Verifier, Signature, VerifyingKey};

    let signature_bytes = hex::decode(&signature_hex).expect("failed to hex-decode signature");
    let pubkey_bytes = hex::decode(&public_key_hex).expect("failed to hex-decode public key");
    let message_bytes = message.as_bytes();

    let signature =
        Signature::try_from(signature_bytes.as_slice()).expect("failed to deserialize signature");
    let is_signature_valid = VerifyingKey::from_sec1_bytes(&pubkey_bytes)
        .expect("failed to deserialize sec1 encoding into public key")
        .verify(message_bytes, &signature)
        .is_ok();

    Ok(SignatureVerificationReply { is_signature_valid })
}

#[query]
async fn y_parity(signature_hex: String, message: String, public_key_hex: String) -> u64 {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    let signature_bytes = hex::decode(&signature_hex).expect("failed to hex-decode signature");
    let signature =
        Signature::try_from(signature_bytes.as_slice()).expect("failed to deserialize signature");

    let pubkey_bytes = hex::decode(&public_key_hex).expect("failed to hex-decode public key");
    let orig_key =
        VerifyingKey::from_sec1_bytes(&pubkey_bytes).expect("failed to parse the pubkey");

    let message_bytes = message.as_bytes();

    for parity in [0u8, 1] {
        let recid = RecoveryId::try_from(parity).unwrap();
        let recovered_key = VerifyingKey::recover_from_msg(&message_bytes, &signature, recid)
            .expect("failed to recover key");

        ic_cdk::println!("parity: {}, recovered_key: {:?}", parity, recovered_key);

        if recovered_key.eq(&orig_key) {
            return parity as u64;
        }
    }

    panic!(
        "failed to recover the parity bit from a signature; sig: {}, pubkey: {}",
        signature_hex, public_key_hex
    )
}

// The fee for the `sign_with_ecdsa` endpoint using the test key.
const SIGN_WITH_ECDSA_COST_CYCLES: u64 = 10_000_000_000;

async fn sign_with_ecdsa(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
) -> Vec<u8> {
    let res: Result<(SignWithECDSAReply,), _> = call_with_payment(
        Principal::management_canister(),
        "sign_with_ecdsa",
        (SignWithECDSA {
            message_hash,
            derivation_path,
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: key_name,
            },
        },),
        SIGN_WITH_ECDSA_COST_CYCLES,
    )
    .await;

    res.unwrap().0.signature
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    let mut output: [u8; 32] = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut output);
    output
}

fn main() {}
ic_cdk_macros::export_candid!();
