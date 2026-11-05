//! Allowlisted timing observations from the already-decoded rollout row.
use super::*;
use serde_json::value::RawValue;

use crate::session_events::{
    Event, Gap, LifecycleKind, Payload as SafePayload, Phase, Position, Time,
};

mod recording;
pub(super) use recording::record;
pub(super) use recording::{Context, ancestry, discontinuity, measurement, operation};

/// A valid outer record binds the damaged item to its known scope and source
/// position. Preserve that boundary without retaining its undecodable payload.
pub(super) fn invalid_item(
    facts: &mut Facts,
    thread: Option<String>,
    turn: Option<String>,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    use crate::session_events::{ContentPhase, ContentPresence, MessageOrigin, MessageRecordKind};
    record(
        facts,
        thread,
        turn,
        SafePayload::Message {
            origin: MessageOrigin::Unknown,
            presence: ContentPresence::Unknown,
            native_id: None,
            record_kind: MessageRecordKind::Unknown,
            record_phase: Phase::Unknown,
            content_phase: ContentPhase::Unknown,
        },
        vec![Gap::SourcePartial],
        report,
        evidence,
    );
}

pub(super) fn safe_integer(raw: &RawValue) -> Option<u64> {
    serde_json::from_str::<u64>(raw.get())
        .ok()
        .filter(|v| *v <= MAX_SAFE_INTEGER)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe(
    payload: &Payload<'_>,
    item: &Payload<'_>,
    kind: &str,
    thread: Option<&str>,
    turn: Option<&str>,
    facts: &mut Facts,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    let mut observations = Vec::new();
    let mut gaps = Vec::new();
    let phase = match kind {
        "task_started" => Some(Phase::Started),
        "task_complete" => Some(Phase::Completed),
        "turn_aborted" => Some(Phase::Cancelled),
        "turn_failed" => Some(Phase::Failed),
        _ => None,
    };
    if let Some(phase) = phase {
        if thread.is_none() || turn.is_none() {
            gaps.push(Gap::MissingIdentity);
        }
        let first_token_ms = payload.time_to_first_token_ms.and_then(safe_integer);
        let duration_ms = payload.duration_ms.and_then(safe_integer);
        if (payload.time_to_first_token_ms.is_some() && first_token_ms.is_none())
            || (payload.duration_ms.is_some() && duration_ms.is_none())
        {
            gaps.push(Gap::InvalidNativeField);
            issue(
                report,
                "invalidTimingField",
                "原生耗时字段无效，保留未知值",
                Some(evidence.clone()),
            );
        }
        observations.push(SafePayload::Lifecycle {
            lifecycle: LifecycleKind::Turn,
            phase,
            native_id: payload.turn_id.clone().filter(|id| !id.is_empty()),
            duration_ms,
            first_token_ms,
        });
    }
    #[derive(Deserialize)]
    struct Info<'a> {
        #[serde(borrow)]
        model_context_window: Option<&'a RawValue>,
    }
    let window_raw = payload.model_context_window.or_else(|| {
        payload
            .info
            .and_then(|raw| serde_json::from_str::<Info>(raw.get()).ok())
            .and_then(|info| info.model_context_window)
    });
    let window = window_raw.and_then(safe_integer).filter(|v| *v > 0);
    if matches!(kind, "task_started" | "token_count") && window_raw.is_some() && window.is_none() {
        gaps.push(Gap::InvalidNativeField);
        issue(
            report,
            "invalidContextWindow",
            "上下文窗口字段无效，保留未知值",
            Some(evidence.clone()),
        );
    }
    if matches!(kind, "task_started" | "token_count")
        && let Some(tokens) = window
    {
        observations.push(SafePayload::ContextWindow {
            model: payload.model.clone(),
            tokens,
        });
    }
    for payload in observations {
        record(
            facts,
            thread.map(str::to_owned),
            turn.map(str::to_owned),
            payload,
            gaps.clone(),
            report,
            evidence,
        );
    }
    items::observe(payload, item, kind, thread, turn, facts, report, evidence);
    messages::observe(payload, item, kind, thread, turn, facts, report, evidence);
}

mod items;
mod messages;
pub(super) use messages::tool_result_content;

mod scopes;
