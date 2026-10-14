//! Safe presence/provenance from Codex 89c8bcf37d64be69e4c8286f4541c1a84ed312a4.
//! Protocol models.rs/items.rs/legacy_events.rs and history/src/lib.rs define
//! these shapes. Field evidence is checked per record, never inferred from CLI version.
use super::*;
use crate::session_events::{ContentPhase, ContentPresence, MessageOrigin, MessageRecordKind};
use serde::de::{SeqAccess, Visitor};
use std::borrow::Cow;
use std::fmt;

#[derive(Deserialize)]
struct HistoryMetadata {
    #[serde(default)]
    inherited_user_message: bool,
}
pub(super) fn history_origin(raw: Option<&RawValue>) -> Option<MessageOrigin> {
    raw.and_then(
        |raw| match serde_json::from_str::<HistoryMetadata>(raw.get()) {
            Ok(metadata) if metadata.inherited_user_message => Some(MessageOrigin::Inherited),
            Ok(_) => None,
            Err(_) => Some(MessageOrigin::Unknown),
        },
    )
}

#[derive(Deserialize)]
struct Metadata<'a> {
    #[serde(borrow)]
    turn_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    content_item_kinds: Option<Vec<Cow<'a, str>>>,
}
#[derive(Deserialize)]
struct Part<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    text: Option<&'a RawValue>,
}
fn text_presence(raw: Option<&RawValue>) -> ContentPresence {
    // RawValue already validates the complete JSON value. A JSON string decodes
    // to empty only for the literal empty string; every valid escape contributes
    // content. Inspect its framing without allocating a decoded body copy.
    let Some(encoded) = raw.map(|raw| raw.get().trim()) else {
        return ContentPresence::Unknown;
    };
    let Some(inner) = encoded
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    else {
        return ContentPresence::Unknown;
    };
    if inner.is_empty() {
        ContentPresence::Empty
    } else {
        ContentPresence::NonEmpty
    }
}

/// Inspect the complete array, releasing each part before the next. Unknown
/// variants cannot prove emptiness, but recognized nonempty text proves presence.
struct Parts(bool);
impl<'de> Visitor<'de> for Parts {
    type Value = ContentPresence;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a message content array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut result = ContentPresence::Empty;
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            let presence = serde_json::from_str::<Part<'_>>(raw.get())
                .ok()
                .filter(|part| {
                    if self.0 {
                        part.kind.as_deref() == Some("Text")
                    } else {
                        matches!(part.kind.as_deref(), Some("input_text" | "output_text"))
                    }
                })
                .map(|part| text_presence(part.text))
                .unwrap_or(ContentPresence::Unknown);
            result = match (result, presence) {
                (ContentPresence::NonEmpty, _) | (_, ContentPresence::NonEmpty) => {
                    ContentPresence::NonEmpty
                }
                (ContentPresence::Unknown, _) | (_, ContentPresence::Unknown) => {
                    ContentPresence::Unknown
                }
                _ => ContentPresence::Empty,
            };
        }
        Ok(result)
    }
}
impl<'de> serde::de::DeserializeSeed<'de> for Parts {
    type Value = ContentPresence;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
fn content_presence(raw: Option<&RawValue>, native: bool) -> ContentPresence {
    use serde::de::DeserializeSeed;
    raw.and_then(|raw| {
        Parts(native)
            .deserialize(&mut serde_json::Deserializer::from_str(raw.get()))
            .ok()
    })
    .unwrap_or(ContentPresence::Unknown)
}

struct SourceParts {
    native: bool,
    row: (usize, usize),
}
impl<'de> Visitor<'de> for SourceParts {
    type Value = crate::conversation_source::OriginalContent;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("message content")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut content = crate::conversation_source::OriginalContent::default();
        let mut index = 0u32;
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            match serde_json::from_str::<Part<'_>>(raw.get()) {
                Ok(part)
                    if if self.native {
                        part.kind.as_deref() == Some("Text")
                    } else {
                        matches!(part.kind.as_deref(), Some("input_text" | "output_text"))
                    } =>
                {
                    if let Some(text) = part.text {
                        content.text(text, self.row, index);
                    } else {
                        content.partial = true;
                    }
                }
                _ => content.partial = true,
            }
            index = index.saturating_add(1);
        }
        Ok(content)
    }
}
impl<'de> serde::de::DeserializeSeed<'de> for SourceParts {
    type Value = crate::conversation_source::OriginalContent;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe(
    p: &Payload<'_>,
    item: &Payload<'_>,
    event: &str,
    thread: Option<&str>,
    turn: Option<&str>,
    facts: &mut Facts,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) {
    let native = matches!(event, "item_started" | "item_completed");
    let kind = item.kind.as_deref().unwrap_or("");
    let (mut origin, presence, record_kind) = match (event, kind, item.role.as_deref()) {
        ("agent_message", _, _) => (
            MessageOrigin::AssistantVisible,
            text_presence(p.message),
            MessageRecordKind::LegacySnapshot,
        ),
        ("agent_message_delta" | "agent_message_content_delta", _, _) => (
            MessageOrigin::AssistantVisible,
            text_presence(p.delta),
            MessageRecordKind::Delta,
        ),
        ("user_message", _, _) if !native => (
            MessageOrigin::UserUnclassified,
            text_presence(p.message),
            MessageRecordKind::LegacySnapshot,
        ),
        ("response_item", "message", role) => (
            match role {
                Some("assistant") => MessageOrigin::AssistantVisible,
                Some("user") => MessageOrigin::UserUnclassified,
                Some("system" | "developer") => MessageOrigin::InjectedContext,
                _ => MessageOrigin::Unknown,
            },
            content_presence(item.content, false),
            MessageRecordKind::ResponseSnapshot,
        ),
        ("response_item", "agent_message", _) => (
            MessageOrigin::InterAgent,
            content_presence(item.content, false),
            MessageRecordKind::ResponseSnapshot,
        ),
        (_, "AgentMessage" | "agentMessage" | "agent_message", _) if native => (
            MessageOrigin::AssistantVisible,
            content_presence(item.content, true),
            MessageRecordKind::NativeSnapshot,
        ),
        (_, "UserMessage" | "userMessage" | "user_message", _) if native => (
            MessageOrigin::UserUnclassified,
            content_presence(item.content, false),
            MessageRecordKind::NativeSnapshot,
        ),
        ("agent_reasoning" | "agent_reasoning_delta", _, _) | ("response_item", "reasoning", _) => {
            (
                MessageOrigin::Reasoning,
                ContentPresence::Unknown,
                MessageRecordKind::Unknown,
            )
        }
        (_, "Reasoning" | "reasoning", _) if native => (
            MessageOrigin::Reasoning,
            ContentPresence::Unknown,
            MessageRecordKind::NativeSnapshot,
        ),
        (_, "ContextCompaction" | "contextCompaction" | "context_compaction", _) if native => (
            MessageOrigin::Compaction,
            ContentPresence::Unknown,
            MessageRecordKind::NativeSnapshot,
        ),
        ("compacted", _, _) | ("response_item", "compaction", _) => (
            MessageOrigin::Compaction,
            ContentPresence::Unknown,
            MessageRecordKind::Unknown,
        ),
        ("response_item", _, _)
            if item.content.is_some() || item.role.as_deref() == Some("assistant") =>
        {
            (
                MessageOrigin::Unknown,
                ContentPresence::Unknown,
                MessageRecordKind::Unknown,
            )
        }
        (_, _, _)
            if native && (item.content.is_some() || super::items::item_kind(kind).is_none()) =>
        {
            (
                MessageOrigin::Unknown,
                ContentPresence::Unknown,
                MessageRecordKind::Unknown,
            )
        }
        _ => return,
    };
    let metadata = item
        .internal_chat_message_metadata_passthrough
        .map(|raw| serde_json::from_str::<Metadata<'_>>(raw.get()));
    if origin == MessageOrigin::UserUnclassified {
        if let Some(Ok(metadata)) = &metadata
            && metadata.content_item_kinds.as_ref().is_some_and(|kinds| {
                !kinds.is_empty()
                    && kinds.iter().all(|kind| {
                        matches!(
                            kind.as_ref(),
                            "agents_md.instructions" | "environments.environment_context"
                        )
                    })
            })
        {
            origin = MessageOrigin::InjectedContext;
        }
    } else if origin == MessageOrigin::AssistantVisible
        && metadata.as_ref().is_some_and(Result::is_err)
    {
        origin = MessageOrigin::Unknown;
    }
    if let Some(history_origin) = facts
        .event_context
        .as_ref()
        .and_then(|context| context.history_origin)
    {
        origin = match history_origin {
            MessageOrigin::Unknown
                if !matches!(
                    origin,
                    MessageOrigin::AssistantVisible | MessageOrigin::Unknown
                ) =>
            {
                origin
            }
            _ => history_origin,
        };
    }
    let mut gaps = Vec::new();
    let metadata_turn = metadata
        .as_ref()
        .and_then(|metadata| metadata.as_ref().ok())
        .and_then(|metadata| metadata.turn_id.as_deref())
        .filter(|id| !id.is_empty());
    let message_turn = metadata_turn
        .zip(thread)
        .map(|(turn, thread)| stable_id(&[thread, "turn", turn]));
    if metadata_turn
        .zip(p.turn_id.as_deref())
        .is_some_and(|(a, b)| a != b)
    {
        gaps.push(Gap::ConflictingIdentity);
    }
    let turn = message_turn.as_deref().or(turn);
    if thread.is_none() || turn.is_none() {
        gaps.push(Gap::MissingIdentity);
    }
    let content_phase = match item
        .phase
        .and_then(|raw| serde_json::from_str::<Cow<'_, str>>(raw.get()).ok())
        .as_deref()
    {
        Some("commentary") => ContentPhase::Commentary,
        Some("final_answer") => ContentPhase::FinalAnswer,
        _ => ContentPhase::Unknown,
    };
    if let Some(context) = &mut facts.event_context
        && let Some(row) = context.source_row
        && record_kind != MessageRecordKind::Unknown
        && !matches!(
            origin,
            MessageOrigin::Reasoning | MessageOrigin::Compaction | MessageOrigin::Unknown
        )
    {
        use serde::de::DeserializeSeed;
        let mut content = crate::conversation_source::OriginalContent::default();
        if let Some(raw) = match record_kind {
            MessageRecordKind::LegacySnapshot => p.message,
            MessageRecordKind::Delta => p.delta,
            _ => None,
        } {
            content.text(raw, row, 0);
        } else if let Some(raw) = item.content {
            content = SourceParts {
                native: native && matches!(kind, "AgentMessage" | "agentMessage" | "agent_message"),
                row,
            }
            .deserialize(&mut serde_json::Deserializer::from_str(raw.get()))
            .unwrap_or(crate::conversation_source::OriginalContent {
                chunks: vec![],
                partial: true,
            });
            if content.chunks.is_empty() && !content.partial {
                content.empty_value(raw, row);
            }
        } else {
            content.partial = true;
        }
        context.original_content = Some(content);
    }
    record(
        facts,
        thread.map(str::to_owned),
        turn.map(str::to_owned),
        SafePayload::Message {
            origin,
            presence,
            native_id: item
                .id
                .as_deref()
                .or(item.item_id.as_deref())
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            record_kind,
            record_phase: match event {
                "item_started" => Phase::Started,
                "item_completed" => Phase::Completed,
                "agent_message_delta" | "agent_message_content_delta" => Phase::Progress,
                _ => Phase::Unknown,
            },
            content_phase,
        },
        gaps,
        report,
        evidence,
    );
}

#[cfg(test)]
mod tests;
