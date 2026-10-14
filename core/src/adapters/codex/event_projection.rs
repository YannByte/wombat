//! Accounting and operation projections consume safe source events, never raw rows.
use super::*;
use crate::session_events::{Event, Payload as SafePayload};

/// Association reads these observations within one archive target, then releases
/// them after producing canonical components.
pub(super) fn association_input(event: &Event) -> bool {
    crate::operation_association::is_projection_input(event)
}
mod replay;
pub(super) use replay::Replay;

/// Match deterministic source traversal, not timestamp or hashed event-ID order.
pub(super) fn order(event: &Event) -> (&str, u64, u32) {
    let evidence = match event.payload() {
        SafePayload::Thread { evidence, .. }
        | SafePayload::Turn { evidence, .. }
        | SafePayload::Ancestry { evidence, .. } => Some(evidence),
        SafePayload::Measurement { value, .. } => value.evidence.first(),
        SafePayload::Operation { value, .. } => value.evidence.first(),
        _ => None,
    };
    (
        evidence.map_or(event.position().file_id.as_ref(), |e| e.file.as_ref()),
        event.position().byte_offset,
        event.position().ordinal,
    )
}

pub(super) fn apply(facts: &mut Facts, event: &Event, report: &mut SourceReport) {
    if let Some(input) = Replay::from_event(event) {
        input.apply(facts, report);
    }
    if association_input(event) {
        facts.operations_pending = true;
    }
}
