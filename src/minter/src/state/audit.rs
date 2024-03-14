pub use super::event::{Event, EventType};
use super::State;
use crate::storage::{record_event, with_event_iter};

/// Updates the state to reflect the given state transition.
// public because it's used in tests since process_event
// requires canister infrastructure to retrieve time
pub fn apply_state_transition(state: &mut State, payload: &EventType) {
    match &payload {
        EventType::Init(init_arg) => {
            panic!("state re-initialization is not allowed: {init_arg:?}");
        }
        EventType::Upgrade(upgrade_arg) => {
            // TODO: when upgrade is done, handle errors
            state.upgrade(upgrade_arg.clone())
            // .expect("applying upgrade event should succeed");
        }
        EventType::SyncedToSignature { signature } => {
            state.record_last_scraped_transaction(signature);
        }
        EventType::SkippedSolSignatureRange { range, reason } => {
            state.record_skipped_signature_range(range.clone());
        }
        EventType::SkippedSolTransaction { sol_tx, reason } => {
            state.record_skipped_transaction(sol_tx.clone());
        }
        EventType::InvalidDeposit { sol_tx, reason } => {
            state.record_invalid_transaction(sol_tx.clone());
        }
        EventType::AcceptedDeposit { deposit, sol_sig } => {
            state.record_accepted_deposit(sol_sig, deposit.clone());
        }
        EventType::MintedDeposit {
            deposit,
            sol_sig,
            icp_mint_block_index,
        } => {
            state.record_minted_deposit(icp_mint_block_index, sol_sig, deposit.clone());
        }
    }
}

/// Records the given event payload in the event log and updates the state to reflect the change.
pub fn process_event(state: &mut State, payload: EventType) {
    apply_state_transition(state, &payload);
    record_event(payload);
}

/// Recomputes the minter state from the event log.
///
/// # Panics
///
/// This function panics if:
///   * The event log is empty.
///   * The first event in the log is not an Init event.
///   * One of the events in the log invalidates the minter's state invariants.
pub fn replay_events() -> State {
    with_event_iter(|mut iter| {
        let mut state = match iter.next().expect("the event log should not be empty") {
            Event {
                payload: EventType::Init(init_arg),
                ..
            } => State::try_from(init_arg).expect("state initialization should succeed"),
            other => panic!("the first event must be an Init event, got: {other:?}"),
        };
        for event in iter {
            apply_state_transition(&mut state, &event.payload);
        }
        state
    })
}
