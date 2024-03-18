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
            // TODO:
            state.upgrade(upgrade_arg.clone())
            // .expect("applying upgrade event should succeed");
        }
        EventType::LastKnownSolanaSignature(signature) => {
            state.record_solana_last_known_signature(signature);
        }
        EventType::NewSolanaSignatureRange(range) => {
            state.record_solana_signature_range(range.clone());
        }
        EventType::RemoveSolanaSignatureRange(range) => {
            state.remove_solana_signature_range(range);
        }
        EventType::RetrySolanaSignatureRange {
            range,
            failed_sub_range,
            fail_reason,
        } => {
            state.retry_solana_signature_range(range.clone(), failed_sub_range.clone());
        }
        EventType::SolanaSignature {
            signature,
            fail_reason,
        } => {
            state.record_solana_signature(signature.clone());
        }
        EventType::InvalidEvent {
            signature,
            fail_reason,
        } => {
            state.record_invalid_event(signature.clone());
        }
        EventType::AcceptedEvent {
            event_source,
            fail_reason,
        } => {
            state.record_accepted_event(event_source.clone());
        }
        EventType::MintedEvent {
            event_source,
            icp_mint_block_index,
        } => {
            state.record_minted_deposit(event_source.clone(), icp_mint_block_index);
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
