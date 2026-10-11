//! Physical native context observations, never historical file loading or occupancy.
use super::*;
use crate::session_events::{ContentPresence, MessageRecordKind, Phase};
pub(super) fn records(
    s: &Snapshot,
    scope: &Scope,
    thread: &crate::usage_store::ThreadEntry,
    rows: &[&PricedMeasurement],
    work: &mut EventWork,
    tz: Tz,
) -> Result<Vec<ContextInventoryRecord>> {
    let mut out = Vec::new();
    let matching_turns: BTreeSet<_> = rows
        .iter()
        .filter_map(|r| r.fact.turn_id.as_deref())
        .collect();
    for event in thread_events(s, thread, work)? {
        let at = event
            .time()
            .timestamp
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|s| s.with_timezone(&tz).date_naive().to_string());
        if scope.undated == Some(true) {
            if at.is_some() {
                continue;
            }
        } else if (scope.since.is_some() || scope.until.is_some())
            && at.as_ref().is_none_or(|at| {
                scope.since.as_ref().is_some_and(|s| at < s)
                    || scope.until.as_ref().is_some_and(|u| at >= u)
            })
        {
            continue;
        }
        if (scope.model.is_some()
            || scope.model_unknown == Some(true)
            || scope.reasoning_effort.is_some()
            || scope.effort_unknown == Some(true))
            && !event
                .turn_id()
                .is_some_and(|turn| matching_turns.contains(turn))
        {
            continue;
        }
        let (kind, record_kind, phase, presence, model, window) = match event.payload() {
            Payload::Message {
                origin: MessageOrigin::InjectedContext,
                presence,
                record_kind,
                record_phase,
                ..
            } => (
                ContextRecordKind::InjectedContext,
                Some(
                    match record_kind {
                        MessageRecordKind::LegacySnapshot => "legacy_snapshot",
                        MessageRecordKind::ResponseSnapshot => "response_snapshot",
                        MessageRecordKind::NativeSnapshot => "native_snapshot",
                        MessageRecordKind::Delta => "delta",
                        MessageRecordKind::Unknown => "unknown",
                    }
                    .into(),
                ),
                Some(
                    match record_phase {
                        Phase::Started => "started",
                        Phase::Progress => "progress",
                        Phase::Completed => "completed",
                        Phase::Failed => "failed",
                        Phase::Cancelled => "cancelled",
                        Phase::Unknown => "unknown",
                    }
                    .into(),
                ),
                Some(
                    match presence {
                        ContentPresence::NonEmpty => "non_empty",
                        ContentPresence::Empty => "empty",
                        ContentPresence::Unknown => "unknown",
                    }
                    .into(),
                ),
                None,
                None,
            ),
            Payload::ContextWindow { model, tokens } => {
                if scope
                    .model
                    .as_ref()
                    .zip(model.as_ref())
                    .is_some_and(|(a, b)| a != b)
                {
                    continue;
                }
                (
                    ContextRecordKind::ModelWindow,
                    None,
                    None,
                    None,
                    model.clone(),
                    Some(*tokens),
                )
            }
            _ => continue,
        };
        out.push(ContextInventoryRecord {
            id: event.id().to_owned(),
            kind,
            timestamp: event.time().timestamp.as_deref().map(str::to_owned),
            record_kind,
            phase,
            presence,
            model,
            model_context_window: window,
            content_version: None,
            bytes: None,
            evidence: evidence(s, scope, &thread.thread.id, event.turn_id(), None),
        });
    }
    Ok(out)
}
