use crate::{
    constants::DERIVATION_PATH,
    events::WithdrawalEvent,
    guard::retrieve_eth_guard,
    logs::DEBUG,
    sol_rpc_client::LedgerMemo,
    state::{audit::process_event, event::EventType, mutate_state, read_state, State},
};

use candid::CandidType;
use candid::Principal;
use ic_cdk::api::{
    call::RejectionCode,
    management_canister::ecdsa::{
        sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, SignWithEcdsaArgument, SignWithEcdsaResponse,
    },
};
use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
use icrc_ledger_types::icrc2::transfer_from::{TransferFromArgs, TransferFromError};
use k256::ecdsa::{signature::Verifier, RecoveryId, Signature, VerifyingKey};
use minicbor::{Decode, Encode};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(CandidType, Debug, Clone, PartialEq, Eq)]
pub enum WithdrawError {
    BurningCkSolFailed(TransferFromError),
    SendingMessageToLedgerFailed {
        id: String,
        code: i32,
        msg: String,
    },
    SigningWithEcdsaFailed {
        burn_id: u64,
        code: RejectionCode,
        msg: String,
    },
    CouponError {
        burn_id: u64,
        err: CouponError,
    },
}

impl std::fmt::Display for WithdrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WithdrawError::BurningCkSolFailed(err) => {
                write!(f, "Failed to burn ckSOL: {err:?}")
            }
            WithdrawError::SendingMessageToLedgerFailed { id, code, msg } => {
                write!(
                    f,
                    "Failed to send a message to the ledger {id}: {code:?}: {msg}",
                )
            }
            WithdrawError::SigningWithEcdsaFailed { burn_id, code, msg } => {
                write!(
                    f,
                    "Failed to sign with ECDSA for burn_id: {burn_id} error: {code:?}: {msg}",
                )
            }
            WithdrawError::CouponError { burn_id, err } => {
                write!(
                    f,
                    "Failed to generate a coupon for burn_id {burn_id} error: {err}"
                )
            }
        }
    }
}

#[derive(CandidType, Debug, Clone, PartialEq, Eq)]
pub enum CouponError {
    HexDecodingError,
    DeserializationError,
    RecoveryError,
    ParityRecoveryFailed { signature: String, pubkey: String },
}

impl std::fmt::Display for CouponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CouponError::HexDecodingError => {
                write!(f, "Failed to hex-decode")
            }
            CouponError::DeserializationError => {
                write!(f, "Failed to deserialize")
            }
            CouponError::RecoveryError => {
                write!(f, "Failed to recover key")
            }
            CouponError::ParityRecoveryFailed { signature, pubkey } => {
                write!(f, "Failed to recover the parity bit from a signature: {signature}, pubkey: {pubkey}")
            }
        }
    }
}

pub async fn withdraw_cksol(
    from: Principal,
    to: String,
    amount: u64,
) -> Result<Coupon, WithdrawError> {
    let _guard = retrieve_eth_guard(from).unwrap_or_else(|e| {
        ic_cdk::trap(&format!(
            "Failed retrieving guard for principal {}: {:?}",
            from, e
        ))
    });

    let mut event = burn_cksol(&from, &to, amount).await.map_err(|err| err)?;
    let coupon = generate_cupon(&mut event).await.map_err(|err| err)?;

    Ok(coupon)
}

async fn burn_cksol(
    from: &Principal,
    to: &String,
    amount: u64,
) -> Result<WithdrawalEvent, WithdrawError> {
    let mut event = WithdrawalEvent::new(
        mutate_state(State::next_burn_id),
        from.clone(),
        to.clone(),
        amount,
    );

    let ledger_canister_id = read_state(|s| s.ledger_id);
    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id,
    };

    let args = TransferFromArgs {
        spender_subaccount: None,
        from: event.from_icp_address.into(),
        to: ic_cdk::id().into(),
        amount: candid::Nat::from(event.amount),
        fee: None,
        created_at_time: Some(ic_cdk::api::time()),
        memo: Some(LedgerMemo(event.get_burn_id()).into()),
    };

    match client.transfer_from(args).await {
        Ok(Ok(block_index)) => {
            let burn_block_index = block_index
                .0
                .to_u64()
                .expect("block index should fit into u64");

            // update event with the burn block index
            event.update_after_burn(ic_cdk::api::time(), burn_block_index);

            process_withdrawal_burn_event(&event, None);

            Ok(event.clone())
        }
        Ok(Err(err)) => Err(WithdrawError::BurningCkSolFailed(err)),
        Err(err) => Err(WithdrawError::SendingMessageToLedgerFailed {
            id: ledger_canister_id.to_string(),
            code: err.0,
            msg: err.1,
        }),
    }
}

async fn generate_cupon(event: &mut WithdrawalEvent) -> Result<Coupon, WithdrawError> {
    match event.to_coupon().await {
        Ok(coupon) => {
            event.update_after_redeem(coupon.clone());
            process_withdrawal_redeem_event(event);
            Ok(coupon)
        }
        Err(err) => {
            process_withdrawal_burn_event(event, Some(err.clone()));
            Err(err)
        }
    }
}

/// Process events
fn process_withdrawal_burn_event(withdraw_event: &WithdrawalEvent, err: Option<WithdrawError>) {
    if let Some(err) = err.clone() {
        ic_canister_log::log!(DEBUG, "{err}");
    }

    mutate_state(|s| {
        process_event(
            s,
            EventType::WithdrawalBurnedEvent {
                event_source: withdraw_event.clone(),
                fail_reason: err.map(|e| e.to_string()),
            },
        )
    });
}

fn process_withdrawal_redeem_event(withdraw_event: &WithdrawalEvent) {
    mutate_state(|s| {
        process_event(
            s,
            EventType::WithdrawalRedeemedEvent {
                event_source: withdraw_event.clone(),
            },
        )
    });
}

/// Types
#[derive(
    CandidType, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, Deserialize, Serialize,
)]
pub struct Coupon {
    #[n(0)]
    pub message: String,
    #[n(1)]
    pub message_hash: String,
    #[n(2)]
    pub signature_hex: String,
    #[n(3)]
    pub icp_public_key_hex: String,
    #[n(4)]
    pub recovery_id: Option<u8>,
}

impl Coupon {
    // Constructor function to create a new Point instance
    pub fn new(
        message: String,
        message_hash: String,
        signature_hex: String,
        icp_public_key_hex: String,
    ) -> Self {
        Self {
            message,
            message_hash,
            signature_hex,
            icp_public_key_hex,
            recovery_id: None,
        }
    }

    pub fn y_parity(&mut self) -> Result<u8, CouponError> {
        let signature_bytes =
            hex::decode(&self.signature_hex).map_err(|_| CouponError::HexDecodingError)?;
        let signature = Signature::try_from(signature_bytes.as_slice())
            .map_err(|_| CouponError::DeserializationError)?;
        let pubkey_bytes =
            hex::decode(&self.icp_public_key_hex).map_err(|_| CouponError::HexDecodingError)?;
        let orig_key = VerifyingKey::from_sec1_bytes(&pubkey_bytes)
            .map_err(|_| CouponError::DeserializationError)?;

        let message_bytes = self.message.as_bytes();

        for parity in [0u8, 1] {
            let rec_id = RecoveryId::try_from(parity).unwrap();
            let recovered_key = VerifyingKey::recover_from_msg(&message_bytes, &signature, rec_id)
                .map_err(|_| CouponError::RecoveryError)?;

            if recovered_key.eq(&orig_key) {
                self.recovery_id = Some(parity);
                return Ok(parity);
            }
        }

        Err(CouponError::ParityRecoveryFailed {
            signature: self.signature_hex.to_string(),
            pubkey: self.icp_public_key_hex.to_string(),
        })
    }

    pub fn verify(&self) -> Result<bool, CouponError> {
        let signature_bytes =
            hex::decode(&self.signature_hex).map_err(|_| CouponError::HexDecodingError)?;
        let pubkey_bytes =
            hex::decode(&self.icp_public_key_hex).map_err(|_| CouponError::HexDecodingError)?;
        let message_bytes = self.message.as_bytes();

        let signature = Signature::try_from(signature_bytes.as_slice())
            .map_err(|_| CouponError::DeserializationError)?;

        Ok(VerifyingKey::from_sec1_bytes(&pubkey_bytes)
            .map_err(|_| CouponError::DeserializationError)?
            .verify(message_bytes, &signature)
            .is_ok())
    }
}

impl WithdrawalEvent {
    pub async fn to_coupon(&self) -> Result<Coupon, WithdrawError> {
        match self.sign_with_ecdsa().await {
            Ok((serialized_coupon, message_hash, signature_hex)) => {
                let icp_public_key_hex = read_state(|s| s.uncompressed_public_key());

                let mut response = Coupon::new(
                    serialized_coupon,
                    message_hash,
                    signature_hex,
                    icp_public_key_hex,
                );

                match response.y_parity() {
                    Ok(_) => Ok(response),
                    Err(err) => {
                        return Err(WithdrawError::CouponError {
                            burn_id: self.get_burn_id(),
                            err,
                        })
                    }
                }
            }
            Err((code, msg)) => Err(WithdrawError::SigningWithEcdsaFailed {
                burn_id: self.get_burn_id(),
                code,
                msg,
            }),
        }
    }

    async fn sign_with_ecdsa(&self) -> Result<(String, String, String), (RejectionCode, String)> {
        // Serialize the coupon
        let serialized_coupon: String = serde_json::to_string(self).unwrap();

        // Hash the serialized coupon using SHA-256
        let mut hasher = Sha256::new();
        hasher.update(serialized_coupon.clone());
        let hashed_coupon = hasher.finalize().to_vec();

        let args = SignWithEcdsaArgument {
            message_hash: hashed_coupon.clone(),
            derivation_path: DERIVATION_PATH.into_iter().map(|x| x.to_vec()).collect(),
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: read_state(|s| s.ecdsa_key_name.clone()),
            },
        };
        let response: Result<(SignWithEcdsaResponse,), (RejectionCode, String)> =
            sign_with_ecdsa(args).await;

        match response {
            Ok(res) => Ok((
                serialized_coupon,
                hex::encode(hashed_coupon),
                hex::encode(&res.0.signature),
            )),
            Err((code, msg)) => Err((code, msg)),
        }
    }
}
