mod types;
use crate::types::{
    Coupon, ECDSAPublicKey, ECDSAPublicKeyReply, EcdsaCurve, EcdsaKeyId, SignWithECDSA,
    SignWithECDSAReply, SignatureVerificationReply,
};
use candid::{candid_method, Nat, Principal};
use ic_cdk::{api::call::call_with_payment, call};
use ic_cdk_macros::{init, query, update};
use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
use icrc_ledger_types::{icrc1::transfer::TransferArg, icrc2::transfer_from::TransferFromArgs};
use num_traits::ToPrimitive;
use serde_json;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use types::InitArgs;

thread_local! {
    // The derivation path to use for ECDSA secp256k1.
    static DERIVATION_PATH: Vec<Vec<u8>> = vec![
        // First component: Hardened derivation for purpose (44')
        vec![0x80, 44],
        // Second component: Hardened derivation for coin type (60')
        vec![0x80, 60],
        // Third component: Hardened derivation for account (0')
        vec![0x80, 0],
        // Fourth component: Non-hardened derivation for external/internal flag (0 for external, 1 for internal)
        vec![0],
        // Fifth component: Non-hardened derivation for index (0)
        vec![1],
    ];

    // The ECDSA key name.
    static KEY_NAME: RefCell<String> = RefCell::new(String::from("dfx_test_key"));

    static LEDGER_CANISTER_ID: RefCell<Principal> = RefCell::new(Principal::from_text("be2us-64aaa-aaaaa-qaabq-cai").unwrap());
}

pub type Address = ethereum_types::H160;

#[candid_method(init)]
#[init]
pub fn init(args: InitArgs) {
    ic_cdk::println!("---> init <---\nledger_id: {}", args.ledger_id);

    LEDGER_CANISTER_ID.with(|id| *id.borrow_mut() = args.ledger_id);
}

#[update]
pub async fn get_address() -> (String, String, String) {
    use libsecp256k1::{PublicKey, PublicKeyFormat};

    let derivation_path = DERIVATION_PATH.with(|d| d.clone());
    let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());

    let public_key = ecdsa_public_key(key_name, derivation_path).await;
    let hex_string = hex::encode(&public_key);

    let uncompressed_pubkey =
        PublicKey::parse_slice(&public_key, Some(PublicKeyFormat::Compressed))
            .expect("failed to deserialize sec1 encoding into public key")
            .serialize();

    let hash = keccak256(&uncompressed_pubkey[1..65]);
    let mut result = [0u8; 20];
    result.copy_from_slice(&hash[12..]);

    return (
        hex_string,
        hex::encode(uncompressed_pubkey),
        hex::encode(result),
    );
}

#[update]
pub async fn sign() -> (String, String, String) {
    let derivation_path = DERIVATION_PATH.with(|d| d.clone());
    let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());

    let coupon = Coupon {
        address: "9gVndQ5SdugdFfGzyuKmePLRJZkCreKZ2iUTEg4agR5g".to_string(),
        amount: 10_000_000_000,
    };

    // Serialize the coupon
    let serialized_coupon: String = serde_json::to_string(&coupon).unwrap();

    // Hash the serialized coupon using SHA-256
    let mut hasher = Sha256::new();
    hasher.update(serialized_coupon.clone());
    let hashed_coupon = hasher.finalize();

    // Convert the hashed coupon into a Vec<u8>
    let hashed_coupon_bytes = hashed_coupon.to_vec();
    let coupon_hex_string = hex::encode(&hashed_coupon_bytes);

    // Sign the hashed coupon using ECDSA
    let signature = sign_with_ecdsa(key_name, derivation_path, hashed_coupon_bytes).await;
    let signature_hex_string = hex::encode(&signature);

    return (serialized_coupon, coupon_hex_string, signature_hex_string);
}

#[update]
async fn mint(user: Principal, amount: Nat) -> Nat {
    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id: LEDGER_CANISTER_ID.with(|d| d.borrow().clone()),
    };

    let args = TransferArg {
        from_subaccount: None,
        to: user.into(),
        fee: None,
        created_at_time: None,
        memo: None,
        amount,
    };

    let block_index: u64 = match client.transfer(args).await {
        Ok(Ok(block_index)) => block_index
            .0
            .to_u64()
            .expect("block index should fit into u64"),
        Ok(Err(err)) => {
            ic_cdk::println!("Failed to mint: {}", err);
            0
        }
        Err(err) => {
            ic_cdk::println!("Failed to mint: {}", err.1);
            0
        }
    };

    Nat::from(block_index)
}

#[update]
async fn burn(amount: Nat) -> Nat {
    let caller = ic_cdk::caller();

    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id: LEDGER_CANISTER_ID.with(|d| d.borrow().clone()),
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

#[query]
async fn get_ledger_id() -> String {
    LEDGER_CANISTER_ID.with(|id| id.borrow().to_text())
}

// The fee for the `sign_with_ecdsa` endpoint using the test key.
const SIGN_WITH_ECDSA_COST_CYCLES: u64 = 10_000_000_000;

/// Returns the ECDSA public key of this canister at the given derivation path.
async fn ecdsa_public_key(key_name: String, derivation_path: Vec<Vec<u8>>) -> Vec<u8> {
    // Retrieve the public key of this canister at the given derivation path
    // from the ECDSA API.
    let res: Result<(ECDSAPublicKeyReply,), _> = call(
        Principal::management_canister(),
        "ecdsa_public_key",
        (ECDSAPublicKey {
            canister_id: None,
            derivation_path,
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: key_name,
            },
        },),
    )
    .await;

    res.unwrap().0.public_key
}

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

ic_cdk_macros::export_candid!();
