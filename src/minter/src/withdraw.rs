use crate::{
    logs::DEBUG,
    state::{audit::process_event, event::EventType, mutate_state, read_state, State},
};

use candid::CandidType;
use candid::Principal;
use icrc_ledger_client_cdk::{CdkRuntime, ICRC1Client};
use icrc_ledger_types::icrc2::transfer_from::TransferFromArgs;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO: add guard
pub async fn withdraw_cksol(from: Principal, to: String, amount: u64) -> Result<Coupon, String> {
    let ledger_canister_id = read_state(|s| s.ledger_id);
    let withdrawal_id = mutate_state(State::next_withdrawal_id);
    let client = ICRC1Client {
        runtime: CdkRuntime,
        ledger_canister_id,
    };

    let args = TransferFromArgs {
        spender_subaccount: None,
        from: from.into(),
        to: ic_cdk::id().into(),
        amount: candid::Nat::from(amount),
        fee: None,
        created_at_time: None,
        // TODO: add memo
        memo: None,
    };

    match client.transfer_from(args).await {
        Ok(Ok(block_index)) => {
            let burn_block_index = block_index
                .0
                .to_u64()
                .expect("block index should fit into u64");

            let withdraw_event = crate::events::WithdrawalEvent {
                id: withdrawal_id,
                from_icp_address: from,
                to_sol_address: to,
                amount,
                timestamp: ic_cdk::api::time(),
                icp_burn_block_index: burn_block_index,
            };

            mutate_state(|s| {
                process_event(
                    s,
                    EventType::WithdrawalEvent {
                        event_source: withdraw_event.clone(),
                    },
                )
            });

            // TODO: in case of failure, we cant revert the state -> maybe a method query that allows regeneration of the coupon is necessary
            let coupon = withdraw_event.to_coupon().await;
            Ok(coupon)
        }
        Ok(Err(err)) => {
            let error_msg = format!("[Withdraw] Failed to burn ckSOL: {err}");
            ic_canister_log::log!(DEBUG, "{}", error_msg);
            Err(error_msg)
        }
        Err(err) => {
            let error_msg = format!(
                "[Withdraw] Failed to send a message to the ledger ({ledger_canister_id}): {err:?}"
            );
            ic_canister_log::log!(DEBUG, "{}", error_msg);
            Err(error_msg)
        }
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct Coupon {
    pub message: String,
    pub signature_hex: String,
    pub icp_public_key_hex: String,
    pub recovery_id: Option<u8>,
}

impl Coupon {
    // Constructor function to create a new Point instance
    pub fn new(message: String, signature_hex: String, icp_public_key_hex: String) -> Self {
        Self {
            message,
            signature_hex,
            icp_public_key_hex,
            recovery_id: None,
        }
    }

    pub fn y_parity(&mut self) {
        use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

        let signature_bytes =
            hex::decode(&self.signature_hex).expect("failed to hex-decode signature");
        let signature = Signature::try_from(signature_bytes.as_slice())
            .expect("failed to deserialize signature");

        let pubkey_bytes =
            hex::decode(&self.icp_public_key_hex).expect("failed to hex-decode public key");
        let orig_key =
            VerifyingKey::from_sec1_bytes(&pubkey_bytes).expect("failed to parse the pubkey");

        let message_bytes = self.message.as_bytes();

        for parity in [0u8, 1] {
            let recid = RecoveryId::try_from(parity).unwrap();
            let recovered_key = VerifyingKey::recover_from_msg(&message_bytes, &signature, recid)
                .expect("failed to recover key");

            ic_cdk::println!("parity: {}, recovered_key: {:?}", parity, recovered_key);

            if recovered_key.eq(&orig_key) {
                self.recovery_id = Some(parity);
                return;
            }
        }

        panic!(
            "failed to recover the parity bit from a signature: sig: {}, pubkey: {}",
            self.signature_hex, self.icp_public_key_hex
        )
    }

    pub fn verify(&self) -> bool {
        use k256::ecdsa::{signature::Verifier, Signature, VerifyingKey};

        let signature_bytes =
            hex::decode(&self.signature_hex).expect("failed to hex-decode signature");
        let pubkey_bytes =
            hex::decode(&self.icp_public_key_hex).expect("failed to hex-decode public key");
        let message_bytes = self.message.as_bytes();

        let signature = Signature::try_from(signature_bytes.as_slice())
            .expect("failed to deserialize signature");

        VerifyingKey::from_sec1_bytes(&pubkey_bytes)
            .expect("failed to deserialize sec1 encoding into public key")
            .verify(message_bytes, &signature)
            .is_ok()
    }
}
