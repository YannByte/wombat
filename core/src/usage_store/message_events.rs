//! Message navigation reuses event frames, retaining original evidence offsets.
//! Borrow non-message payloads instead of constructing and validating their facts.
use super::*;
use crate::session_events::{Event, Payload, Position};
use serde::de::{SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{borrow::Cow, fmt, sync::atomic::AtomicBool};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageTimeRange {
    pub first: Option<String>,
    pub last: Option<String>,
    pub undated: bool,
}
impl MessageTimeRange {
    pub(crate) fn build(events: &[Arc<Event>]) -> Self {
        let mut range = Self::default();
        for event in events {
            if !matches!(event.payload(), Payload::Message { .. }) {
                continue;
            }
            let time = event.time().timestamp.as_deref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| {
                    t.to_utc()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
                })
            });
            if let Some(time) = time {
                if range.first.as_ref().is_none_or(|first| time < *first) {
                    range.first = Some(time.clone());
                }
                if range.last.as_ref().is_none_or(|last| time > *last) {
                    range.last = Some(time);
                }
            } else {
                range.undated = true;
            }
        }
        range
    }
    pub(super) fn validate(&self, count: usize) -> Result<()> {
        let valid_time = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .is_some_and(|t| {
                    t.to_utc()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
                        == s
                })
        };
        if self.first.is_some() != self.last.is_some()
            || self.first.as_deref().is_some_and(|s| !valid_time(s))
            || self.last.as_deref().is_some_and(|s| !valid_time(s))
            || self
                .first
                .as_ref()
                .zip(self.last.as_ref())
                .is_some_and(|(a, b)| a > b)
            || count == 0 && self != &Self::default()
            || count > 0 && self.first.is_none() && !self.undated
        {
            return Err(corrupt("message time summary invalid"));
        }
        Ok(())
    }
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
struct BorrowedPosition<'a> {
    #[serde(borrow)]
    source_instance_id: Cow<'a, str>,
    #[serde(borrow)]
    file_id: Cow<'a, str>,
    #[serde(borrow)]
    generation: Cow<'a, str>,
    byte_offset: u64,
    ordinal: u32,
}
impl BorrowedPosition<'_> {
    fn owned(&self) -> Position {
        Position {
            source_instance_id: self.source_instance_id.as_ref().into(),
            file_id: self.file_id.as_ref().into(),
            generation: self.generation.as_ref().into(),
            byte_offset: self.byte_offset,
            ordinal: self.ordinal,
        }
    }
}
#[derive(Deserialize)]
struct Kind<'a> {
    #[serde(borrow)]
    kind: Cow<'a, str>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header<'a> {
    version: u32,
    #[serde(borrow)]
    position: BorrowedPosition<'a>,
    #[serde(borrow)]
    thread_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    turn_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    payload: Kind<'a>,
}

pub(crate) struct MessageRows {
    count: usize,
    first: Option<Position>,
    last: Option<Position>,
    target: EventTarget,
    pub rows: Vec<(usize, Arc<Event>)>,
}
impl<'de> Deserialize<'de> for MessageRows {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Messages;
        impl<'de> Visitor<'de> for Messages {
            type Value = MessageRows;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("one bounded event frame")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                use serde::de::Error;
                let mut out = MessageRows {
                    count: 0,
                    first: None,
                    last: None,
                    target: EventTarget {
                        thread_id: None,
                        turn_id: None,
                    },
                    rows: vec![],
                };
                let mut prior: Option<Header<'_>> = None;
                while let Some(raw) = seq.next_element::<&RawValue>()? {
                    if out.count >= MAX_EVENT_PAGE_ROWS {
                        return Err(A::Error::custom("event frame count exceeded"));
                    }
                    let header: Header<'_> =
                        serde_json::from_str(raw.get()).map_err(A::Error::custom)?;
                    if header.version != crate::session_events::EVENT_VERSION {
                        return Err(A::Error::custom("unsupported event version"));
                    }
                    if let Some(prior) = &prior {
                        if prior.thread_id != header.thread_id
                            || prior.turn_id != header.turn_id
                            || prior.position >= header.position
                        {
                            return Err(A::Error::custom("event frame target or order mismatch"));
                        }
                    } else {
                        out.first = Some(header.position.owned());
                        out.target = EventTarget {
                            thread_id: header.thread_id.as_deref().map(str::to_owned),
                            turn_id: header.turn_id.as_deref().map(str::to_owned),
                        };
                    }
                    if header.payload.kind == "message" {
                        let event = serde_json::from_str(raw.get()).map_err(A::Error::custom)?;
                        out.rows.push((out.count, event));
                    }
                    out.count += 1;
                    prior = Some(header);
                }
                out.last = prior.map(|h| h.position.owned());
                Ok(out)
            }
        }
        d.deserialize_seq(Messages)
    }
}

impl Snapshot {
    /// At most one existing frame is decoded. Offsets still count every archived
    /// event, so raw pages, context windows and previously returned evidence agree.
    pub(crate) fn message_chunk(
        &self,
        partition: &EventPartition,
        chunk: &EventChunk,
        cancelled: &AtomicBool,
    ) -> Result<Vec<(usize, Arc<Event>)>> {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(operation_error("CANCELLED", "message query cancelled"));
        }
        let rows = if let Some(buckets) = &self.live_events {
            let all = buckets
                .get(&partition.target)
                .ok_or_else(|| corrupt("missing event target"))?;
            let selected = all
                .get(chunk.offset..chunk.offset + chunk.count)
                .ok_or_else(|| corrupt("event frame range invalid"))?;
            MessageRows {
                count: selected.len(),
                first: selected.first().map(|e| e.position().clone()),
                last: selected.last().map(|e| e.position().clone()),
                target: partition.target.clone(),
                rows: selected
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| matches!(e.payload(), Payload::Message { .. }))
                    .map(|(i, e)| (i, Arc::clone(e)))
                    .collect(),
            }
        } else if let Some(cold) = &self.cold_events {
            cold.read_messages(chunk)?
        } else {
            #[derive(Deserialize)]
            struct Block {
                version: u32,
                target: EventTarget,
                events: MessageRows,
            }
            let bytes = super::events::read_block_bytes(&self.directory, chunk, cancelled)?;
            let block: Block =
                serde_json::from_slice(&bytes).map_err(|e| corrupt(e.to_string()))?;
            if block.version != super::events::EVENT_INDEX_VERSION {
                return Err(operation_error(
                    "UNSUPPORTED_VERSION",
                    "unsupported event block version",
                ));
            }
            if block.target != partition.target {
                return Err(corrupt("event frame target mismatch"));
            }
            block.events
        };
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(operation_error("CANCELLED", "message query cancelled"));
        }
        if rows.count != chunk.count
            || rows.rows.len() != chunk.message_count
            || rows.target != partition.target
            || rows.first.as_ref() != Some(&chunk.first)
            || rows.last.as_ref() != Some(&chunk.last)
        {
            return Err(corrupt("message frame count, target or boundary mismatch"));
        }
        Ok(rows
            .rows
            .into_iter()
            .map(|(i, e)| (chunk.offset + i, e))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::query;
    use crate::session_events::MessageOrigin;

    #[test]
    fn time_summaries_normalize_offsets_keep_undated_messages_and_reject_invalid_bounds() {
        let make = |offset, at| {
            Arc::new(
                Event::new(
                    Position {
                        source_instance_id: "s".into(),
                        file_id: "f".into(),
                        generation: "g".into(),
                        byte_offset: offset,
                        ordinal: 0,
                    },
                    None,
                    None,
                    crate::session_events::Time::from_source(at).0,
                    vec![],
                    Payload::Message {
                        origin: MessageOrigin::UserUnclassified,
                        presence: crate::session_events::ContentPresence::NonEmpty,
                        native_id: None,
                        record_kind: crate::session_events::MessageRecordKind::ResponseSnapshot,
                        record_phase: crate::session_events::Phase::Completed,
                        content_phase: crate::session_events::ContentPhase::Unknown,
                    },
                )
                .unwrap(),
            )
        };
        let rows = vec![
            make(0, Some("2026-10-14T01:00:00+08:00")),
            make(1, None),
            make(2, Some("2026-10-13T16:00:00Z")),
        ];
        let range = MessageTimeRange::build(&rows);
        assert_eq!(
            range.first.as_deref(),
            Some("2026-10-13T16:00:00.000000000Z")
        );
        assert_eq!(
            range.last.as_deref(),
            Some("2026-10-13T17:00:00.000000000Z")
        );
        assert!(range.undated);
        range.validate(3).unwrap();
        assert!(range.validate(0).is_err());
        let encoded = serde_json::to_value(&rows).unwrap();
        let parsed: MessageRows =
            serde_json::from_slice(&serde_json::to_vec(&encoded).unwrap()).unwrap();
        assert_eq!(parsed.count, 3);
        assert_eq!(parsed.rows.len(), 3);
        for change in ["version", "target", "order", "message"] {
            let mut damaged = encoded.clone();
            match change {
                "version" => damaged[1]["version"] = serde_json::json!(99),
                "target" => damaged[1]["threadId"] = serde_json::json!("other"),
                "order" => damaged.as_array_mut().unwrap().swap(0, 2),
                "message" => damaged[1]["payload"]["origin"] = serde_json::json!("future"),
                _ => unreachable!(),
            }
            assert!(
                serde_json::from_slice::<MessageRows>(&serde_json::to_vec(&damaged).unwrap())
                    .is_err(),
                "{change}"
            );
        }
        let mut invalid = range.clone();
        invalid.first = Some("invalid".into());
        assert!(invalid.validate(3).is_err());
        let mut invalid = range.clone();
        std::mem::swap(&mut invalid.first, &mut invalid.last);
        assert!(invalid.validate(3).is_err());
    }
    #[test]
    fn message_navigation_skips_control_frames_preserves_offsets_and_explicit_context() {
        use crate::session_events::{
            ContentPhase, ContentPresence, MessageRecordKind, Position, Time,
        };
        let root = tempfile::tempdir().unwrap();
        let mut data = crate::adapters::contract::Collected::default();
        for i in 0..6000 {
            data.events.push(Arc::new(
                Event::new(
                    Position {
                        source_instance_id: "source".into(),
                        file_id: "file".into(),
                        generation: "generation".into(),
                        byte_offset: i,
                        ordinal: 0,
                    },
                    None,
                    None,
                    Time::from_source(Some("2026-10-14T00:00:00Z")).0,
                    vec![],
                    if i == 4100 || i == 4200 {
                        Payload::Message {
                            origin: if i == 4100 {
                                MessageOrigin::AssistantVisible
                            } else {
                                MessageOrigin::InjectedContext
                            },
                            presence: ContentPresence::NonEmpty,
                            native_id: None,
                            record_kind: MessageRecordKind::ResponseSnapshot,
                            record_phase: crate::session_events::Phase::Completed,
                            content_phase: ContentPhase::FinalAnswer,
                        }
                    } else {
                        Payload::Activity {
                            activity: crate::session_events::ActivityKind::Tool,
                        }
                    },
                )
                .unwrap(),
            ));
        }
        let memory = crate::usage_store::memory(
            data.clone(),
            "memory".into(),
            crate::pricing_sync::current_at(root.path()).unwrap(),
            None,
        )
        .unwrap();
        let disk = super::super::files::save_at(root.path(), data).unwrap();
        for snapshot in [&memory, &disk] {
            let mut r: crate::conversation_dto::Request = serde_json::from_value(serde_json::json!({"action":"locate","snapshotId":snapshot.manifest.snapshot_ref.snapshot_id})).unwrap();
            let complete = |r: &crate::conversation_dto::Request| {
                let mut r = r.clone();
                let mut response = query(snapshot, None, &r, &AtomicBool::new(false)).unwrap();
                for _ in 0..100 {
                    if response.scan_complete {
                        return response;
                    }
                    r.cursor = response.next_cursor.clone();
                    let next = query(snapshot, None, &r, &AtomicBool::new(false)).unwrap();
                    response.items.extend(next.items);
                    response.scanned_events += next.scanned_events;
                    response.scan_complete = next.scan_complete;
                    response.next_cursor = next.next_cursor;
                }
                panic!("message scan failed to advance");
            };
            let first = complete(&r);
            assert!(first.scan_complete);
            assert_eq!(first.scanned_events, 2);
            assert_eq!(first.items.len(), 1);
            assert_eq!(first.items[0].evidence.event_offset, 4100);
            r.selection.origins = vec![MessageOrigin::InjectedContext];
            let injected = complete(&r);
            assert_eq!(injected.items.len(), 1);
            assert_eq!(injected.items[0].evidence.event_offset, 4200);
            let mut anchored = r.clone();
            anchored.anchor = Some(injected.items[0].evidence.clone());
            anchored.radius = Some(0);
            let window = query(snapshot, None, &anchored, &AtomicBool::new(false)).unwrap();
            assert_eq!(window.items[0].evidence.event_offset, 4200);
            r.selection.origins.clear();
            r.selection.since = Some("2026-10-15".into());
            let empty = complete(&r);
            assert!(empty.scan_complete);
            assert_eq!(empty.scanned_events, 0);
            assert!(empty.items.is_empty());
        }
    }
}
