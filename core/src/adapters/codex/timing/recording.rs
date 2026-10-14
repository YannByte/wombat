//! One source row allocates a single ordered namespace for all safe observations.
use super::*;

pub(in crate::adapters::codex) struct Context {
    pub(super) evidence: EvidenceRef,
    position: Position,
    time: Time,
    collected_at: String,
    gaps: Vec<Gap>,
    phase: Phase,
    pub(super) history_origin: Option<crate::session_events::MessageOrigin>,
    pub(super) source_row: Option<(usize, usize)>,
    pub(super) original_content: Option<crate::conversation_source::OriginalContent>,
}
impl Context {
    pub(in crate::adapters::codex) fn locally_owned(&self) -> bool {
        self.history_origin.is_none()
            || self.history_origin == Some(crate::session_events::MessageOrigin::AssistantVisible)
    }
    pub(in crate::adapters::codex) fn new(
        position: Position,
        timestamp: Option<&str>,
        kind: &str,
        subtype: Option<&str>,
        evidence: &EvidenceRef,
        metadata: Option<&RawValue>,
    ) -> Self {
        let (time, gap) = Time::from_source(timestamp);
        let event = if kind == "event_msg" || kind == "response_item" {
            subtype.unwrap_or("")
        } else {
            kind
        };
        let phase = match event {
            "item_started" | "mcp_tool_call_begin" | "function_call" | "custom_tool_call" => {
                Phase::Started
            }
            "item_completed"
            | "patch_apply_end"
            | "mcp_tool_call_end"
            | "function_call_output"
            | "custom_tool_call_output" => Phase::Completed,
            _ => Phase::Unknown,
        };
        Self {
            evidence: evidence.clone(),
            position,
            time,
            collected_at: chrono::Utc::now().to_rfc3339(),
            gaps: gap.into_iter().collect(),
            phase,
            history_origin: super::messages::history_origin(metadata),
            source_row: None,
            original_content: None,
        }
    }
    pub(in crate::adapters::codex) fn source_row(mut self, row: &[u8]) -> Self {
        self.source_row = Some((row.as_ptr() as usize, row.len()));
        self
    }
}

/// A corrupt complete row breaks attribution but is not a target-turn sample.
/// Reuse the same source namespace; no timestamp can be trusted from that row.
pub(in crate::adapters::codex) fn discontinuity(
    facts: &mut Facts,
    position: Position,
    thread: Option<String>,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    use crate::session_events::{ContentPhase, ContentPresence, MessageOrigin, MessageRecordKind};
    let previous = facts.event_context.replace(Context::new(
        position,
        None,
        "source_gap",
        None,
        evidence,
        None,
    ));
    record(
        facts,
        thread,
        None,
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
    facts.event_context = previous;
}

#[allow(clippy::too_many_arguments)]
pub(in crate::adapters::codex) fn record(
    facts: &mut Facts,
    thread: Option<String>,
    turn: Option<String>,
    payload: SafePayload,
    mut gaps: Vec<Gap>,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    let Some(context) = &mut facts.event_context else {
        issue(
            report,
            "missingEventContext",
            "事件缺少来源位置，无法建立投影",
            Some(evidence.clone()),
        );
        return;
    };
    for gap in &context.gaps {
        if !gaps.contains(gap) {
            gaps.push(gap.clone());
        }
    }
    let position = context.position.clone();
    context.position.ordinal += 1;
    let collected_at = position
        .event_id()
        .ok()
        .and_then(|id| {
            facts
                .retained_collection_times
                .remove(id.as_str())
                .or_else(|| {
                    facts
                        .events
                        .get(id.as_str())
                        .map(|e| e.collected_at().to_owned())
                })
        })
        .unwrap_or_else(|| context.collected_at.clone());
    match Event::new_at(
        position,
        thread,
        turn,
        context.time.clone(),
        gaps,
        payload,
        collected_at,
    ) {
        Ok(mut event) => {
            if matches!(event.payload(), SafePayload::Message { .. })
                && let Some(content) = context.original_content.take()
                && event.set_original_content(content).is_err()
            {
                issue(
                    report,
                    "invalidSourceField",
                    "原文位置无效",
                    Some(evidence.clone()),
                );
                return;
            }
            facts.event_strings.share(&mut event);
            crate::adapters::codex::event_projection::apply(facts, &event, report);
            let id = event.shared_id();
            if !facts.full_event_write && !facts.streaming {
                facts.dirty_events.insert(id.clone());
            }
            let event = Arc::new(event);
            if facts.streaming {
                // Canonical facts are applied now; associations read only affected
                // archive scopes after these bounded batches have been flushed.
                facts.pending_events.push(event);
            } else {
                facts.events.insert(id, event);
            }
        }
        Err(_) => issue(
            report,
            "invalidEventIdentity",
            "事件关联身份不完整",
            Some(evidence.clone()),
        ),
    }
}

pub(in crate::adapters::codex) fn operation(
    facts: &mut Facts,
    operation: &Operation,
    report: &mut SourceReport,
) {
    let Some(context) = &facts.event_context else {
        issue(
            report,
            "missingEventContext",
            "事件缺少来源位置，无法建立投影",
            operation.evidence.first().cloned(),
        );
        return;
    };
    let phase = match operation.status.as_ref() {
        "failed" => Phase::Failed,
        "interrupted" => Phase::Cancelled,
        "completed" if context.phase != Phase::Unknown => Phase::Completed,
        _ => context.phase.clone(),
    };
    let Some(evidence) = operation.evidence.first() else {
        return;
    };
    record(
        facts,
        Some(operation.thread_id.to_string()),
        operation.turn_id.as_deref().map(str::to_owned),
        SafePayload::Operation {
            value: Arc::new(operation.clone()),
            phase,
        },
        vec![],
        report,
        evidence,
    );
}

pub(in crate::adapters::codex) fn measurement(
    facts: &mut Facts,
    candidate: &Candidate,
    context_conflicts: &[crate::session_events::MeasurementContextField],
    report: &mut SourceReport,
) {
    let mut value = candidate.measurement.clone();
    if !context_conflicts.is_empty() {
        super::super::context::clear_fields(Arc::make_mut(&mut value), context_conflicts);
    }
    let Some(evidence) = value.evidence.first() else {
        return;
    };
    record(
        facts,
        value.thread_id.as_deref().map(str::to_owned),
        value.turn_id.as_deref().map(str::to_owned),
        SafePayload::Measurement {
            value: value.clone(),
            context_conflicts: context_conflicts.to_vec(),
            direct: candidate.direct,
            cumulative: candidate.cumulative,
            interval_start: candidate.interval_start,
            fingerprint: candidate.fingerprint.clone(),
        },
        vec![],
        report,
        evidence,
    );
}

pub(in crate::adapters::codex) fn ancestry(
    facts: &mut Facts,
    thread: &str,
    parent_id: String,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    record(
        facts,
        Some(thread.into()),
        None,
        SafePayload::Ancestry {
            parent_id,
            evidence: evidence.clone(),
        },
        vec![],
        report,
        evidence,
    );
}
