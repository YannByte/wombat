//! Safe, versioned source facts. Source bodies cannot be represented in this model.
//! Storage and projections share these identities; public query DTOs remain separate.
use crate::adapters::contract::{
    EvidenceRef, MAX_SAFE_INTEGER, Measurement, Operation, Thread, Turn,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const EVENT_VERSION: u32 = 4;
mod review;
pub use review::{ReviewObservation, SafetyLabel};
pub mod title_observations;

/// A generation belongs to one physical source file, not to an entire source root.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
    pub source_instance_id: Arc<str>,
    pub file_id: Arc<str>,
    pub generation: Arc<str>,
    pub byte_offset: u64,
    pub ordinal: u32,
}
impl Position {
    pub fn event_id(&self) -> Result<String> {
        ensure!(
            !self.source_instance_id.is_empty()
                && !self.file_id.is_empty()
                && !self.generation.is_empty(),
            "event source identity is incomplete"
        );
        // Structured encoding keeps delimiter characters in native identities unambiguous.
        Ok(crate::hash(serde_json::to_vec(self)?))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Time {
    pub timestamp: Option<Arc<str>>,
    pub precision: Precision,
}
impl Time {
    /// Invalid timestamps are an explicit gap; they never discard an otherwise valid fact.
    pub fn from_source(value: Option<&str>) -> (Self, Option<Gap>) {
        let parsed = value.and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok());
        let Some(parsed) = parsed else {
            return (
                Self {
                    timestamp: None,
                    precision: Precision::Unknown,
                },
                value.map(|_| Gap::InvalidTimestamp),
            );
        };
        let digits = value
            .and_then(|text| text.split_once('.'))
            .map_or(0, |(_, tail)| {
                tail.chars().take_while(char::is_ascii_digit).count()
            });
        let precision = match digits {
            0 => Precision::Second,
            1..=3 => Precision::Millisecond,
            4..=6 => Precision::Microsecond,
            _ => Precision::Nanosecond,
        };
        (
            Self {
                timestamp: Some(
                    parsed
                        .to_utc()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
                        .into(),
                ),
                precision,
            },
            None,
        )
    }
    fn validate(&self) -> Result<()> {
        if let Some(time) = &self.timestamp {
            ensure!(
                chrono::DateTime::parse_from_rfc3339(time).is_ok(),
                "invalid event timestamp"
            );
            ensure!(
                self.precision != Precision::Unknown,
                "known timestamp needs precision"
            );
        } else {
            ensure!(
                self.precision == Precision::Unknown,
                "missing timestamp cannot have precision"
            );
        }
        Ok(())
    }
}

/// Field-level contradictions in one source measurement, separate from time/identity gaps.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeasurementContextField {
    Model,
    Provider,
    ApiProvider,
    Effort,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gap {
    InvalidTimestamp,
    InvalidNativeField,
    MissingIdentity,
    ConflictingIdentity,
    UnmatchedBoundary,
    SourcePartial,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleKind {
    Turn,
    ModelRequest,
    Compaction,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Assistant,
    Reasoning,
    Tool,
}

/// Source-backed provenance only; an unclassified user record is not authorization.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageOrigin {
    AssistantVisible,
    UserInput,
    UserUnclassified,
    InjectedContext,
    Reasoning,
    InterAgent,
    Inherited,
    Compaction,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentPresence {
    NonEmpty,
    Empty,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRecordKind {
    LegacySnapshot,
    ResponseSnapshot,
    NativeSnapshot,
    Delta,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentPhase {
    Commentary,
    FinalAnswer,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Assistant,
    Reasoning,
    User,
    Command,
    File,
    Mcp,
    Tool,
    Compaction,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeDuration {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payload {
    Review {
        observation: ReviewObservation,
    },
    Ancestry {
        parent_id: String,
        evidence: EvidenceRef,
    },
    Thread {
        value: Box<Thread<Arc<str>>>,
        project_path: Option<Arc<str>>,
        evidence: EvidenceRef,
    },
    Turn {
        value: Box<Turn<Arc<str>>>,
        evidence: EvidenceRef,
    },
    /// Preserve source accounting inputs before cross-file deduplication and reconciliation.
    Measurement {
        value: Arc<Measurement>,
        #[serde(rename = "contextConflicts")]
        context_conflicts: Vec<MeasurementContextField>,
        direct: bool,
        cumulative: Option<u64>,
        interval_start: Option<u64>,
        fingerprint: String,
    },
    Operation {
        value: Arc<Operation>,
        phase: Phase,
    },
    Lifecycle {
        lifecycle: LifecycleKind,
        phase: Phase,
        native_id: Option<String>,
        duration_ms: Option<u64>,
        first_token_ms: Option<u64>,
    },
    Activity {
        activity: ActivityKind,
    },
    /// Presence is observed in memory; no body, size, hash or media reference is retained.
    Message {
        origin: MessageOrigin,
        presence: ContentPresence,
        native_id: Option<String>,
        record_kind: MessageRecordKind,
        record_phase: Phase,
        content_phase: ContentPhase,
    },
    Item {
        item_kind: ItemKind,
        native_id: Option<String>,
        phase: Phase,
        started_at_ms: Option<i64>,
        completed_at_ms: Option<i64>,
        duration: Option<NativeDuration>,
    },
    ContextWindow {
        model: Option<String>,
        tokens: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredEvent {
    version: u32,
    id: Arc<str>,
    position: Position,
    thread_id: Option<Arc<str>>,
    turn_id: Option<Arc<str>>,
    time: Time,
    collected_at: Arc<str>,
    gaps: Vec<Gap>,
    payload: Payload,
}

/// Constructors and deserialization both enforce the same current-format envelope.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "StoredEvent")]
pub struct Event(StoredEvent);
#[derive(Default)]
pub(crate) struct EventStrings {
    domains: crate::shared_text::SharedText,
    scopes: crate::shared_text::SharedText,
    // Adjacent observations commonly repeat row time and scope. Keep one value
    // per field after the bounded general pools fill, without growing a cache.
    thread: RecentText,
    turn: RecentText,
    collected: RecentText,
    timestamp: RecentText,
    thread_fields: [RecentText; 6],
    turn_fields: [RecentText; 4],
    paths: crate::shared_text::SharedText,
    facts: crate::adapters::shared_strings::FactStrings,
}
impl EventStrings {
    pub(crate) fn share(&mut self, event: &mut Event) {
        for value in [
            &mut event.0.position.source_instance_id,
            &mut event.0.position.file_id,
            &mut event.0.position.generation,
        ] {
            self.domains.share(value);
        }
        for (value, recent) in [
            (&mut event.0.thread_id, &mut self.thread),
            (&mut event.0.turn_id, &mut self.turn),
        ] {
            if let Some(value) = value {
                self.scopes.share(value);
                recent.share(value);
            }
        }
        self.collected.share(&mut event.0.collected_at);
        if let Some(value) = &mut event.0.time.timestamp {
            self.timestamp.share(value);
        }
        match &mut event.0.payload {
            Payload::Thread {
                value,
                project_path,
                evidence,
            } => {
                for (field, recent) in [
                    Some(&mut value.id),
                    Some(&mut value.agent_kind),
                    Some(&mut value.source_instance_id),
                    Some(&mut value.upstream_id),
                    value.title.as_mut(),
                    value.project.as_mut(),
                ]
                .into_iter()
                .zip(&mut self.thread_fields)
                {
                    if let Some(field) = field {
                        self.scopes.share(field);
                        recent.share(field);
                    }
                }
                value.id = Arc::clone(event.0.thread_id.as_ref().expect("validated thread scope"));
                value.source_instance_id = Arc::clone(&event.0.position.source_instance_id);
                if let (Some(path), Some(project)) = (project_path, &value.project)
                    && path == project
                {
                    *path = Arc::clone(project);
                }
                for time in [&mut value.started_at, &mut value.last_activity_at]
                    .into_iter()
                    .flatten()
                {
                    self.timestamp.share(time);
                }
                self.paths.share(&mut evidence.file);
            }
            Payload::Turn { value, evidence } => {
                for (field, recent) in [
                    &mut value.id,
                    &mut value.thread_id,
                    &mut value.upstream_id,
                    &mut value.status,
                ]
                .into_iter()
                .zip(&mut self.turn_fields)
                {
                    self.scopes.share(field);
                    recent.share(field);
                }
                value.id = Arc::clone(event.0.turn_id.as_ref().expect("validated turn scope"));
                value.thread_id =
                    Arc::clone(event.0.thread_id.as_ref().expect("validated thread scope"));
                for time in [
                    &mut value.started_at,
                    &mut value.ended_at,
                    &mut value.last_activity_at,
                ]
                .into_iter()
                .flatten()
                {
                    self.timestamp.share(time);
                }
                self.paths.share(&mut evidence.file);
            }
            Payload::Ancestry { evidence, .. } => self.paths.share(&mut evidence.file),
            Payload::Measurement { value, .. } => {
                if let Some(value) = Arc::get_mut(value) {
                    self.facts.measurement(value);
                    for evidence in &mut value.evidence {
                        self.paths.share(&mut evidence.file);
                    }
                }
            }
            Payload::Operation { value, .. } => {
                if let Some(value) = Arc::get_mut(value) {
                    self.facts.operation(value);
                    for evidence in &mut value.evidence {
                        self.paths.share(&mut evidence.file);
                    }
                }
            }
            _ => {}
        }
    }
}
#[derive(Default)]
struct RecentText(Option<Arc<str>>);
impl RecentText {
    fn share(&mut self, value: &mut Arc<str>) {
        if value.len() > 1024 {
            return;
        }
        if let Some(previous) = &self.0
            && previous.as_ref() == value.as_ref()
        {
            *value = Arc::clone(previous);
        } else {
            self.0 = Some(Arc::clone(value));
        }
    }
}
impl Serialize for Event {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
impl Event {
    pub fn new(
        position: Position,
        thread_id: Option<String>,
        turn_id: Option<String>,
        time: Time,
        gaps: Vec<Gap>,
        payload: Payload,
    ) -> Result<Self> {
        Self::new_at(
            position,
            thread_id,
            turn_id,
            time,
            gaps,
            payload,
            chrono::Utc::now().to_rfc3339(),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_at(
        position: Position,
        thread_id: Option<String>,
        turn_id: Option<String>,
        time: Time,
        gaps: Vec<Gap>,
        payload: Payload,
        collected_at: String,
    ) -> Result<Self> {
        let id = position.event_id()?;
        Self::validate_fields(StoredEvent {
            version: EVENT_VERSION,
            id: id.into(),
            position,
            thread_id: thread_id.map(Into::into),
            turn_id: turn_id.map(Into::into),
            time,
            collected_at: collected_at.into(),
            gaps,
            payload,
        })
    }
    pub fn id(&self) -> &str {
        &self.0.id
    }
    pub(crate) fn shared_id(&self) -> Arc<str> {
        Arc::clone(&self.0.id)
    }
    pub fn position(&self) -> &Position {
        &self.0.position
    }
    pub fn payload(&self) -> &Payload {
        &self.0.payload
    }
    pub fn time(&self) -> &Time {
        &self.0.time
    }
    pub fn collected_at(&self) -> &str {
        &self.0.collected_at
    }
    pub fn thread_id(&self) -> Option<&str> {
        self.0.thread_id.as_deref()
    }
    pub(crate) fn shared_target(&self) -> (Option<Arc<str>>, Option<Arc<str>>) {
        (self.0.thread_id.clone(), self.0.turn_id.clone())
    }
    pub fn turn_id(&self) -> Option<&str> {
        self.0.turn_id.as_deref()
    }
    pub fn gaps(&self) -> &[Gap] {
        &self.0.gaps
    }
}
impl TryFrom<StoredEvent> for Event {
    type Error = anyhow::Error;
    fn try_from(value: StoredEvent) -> Result<Self> {
        ensure!(value.version == EVENT_VERSION, "unsupported event version");
        ensure!(
            value.id.as_ref() == value.position.event_id()?,
            "event identity mismatch"
        );
        Self::validate_fields(value)
    }
}
impl Event {
    // Constructors compute the identity once; persisted envelopes verify it before
    // entering the same semantic validation path.
    fn validate_fields(value: StoredEvent) -> Result<Self> {
        ensure!(
            chrono::DateTime::parse_from_rfc3339(&value.collected_at).is_ok(),
            "invalid event collection time"
        );
        ensure!(
            value.turn_id.is_none() || value.thread_id.is_some(),
            "turn requires thread identity"
        );
        ensure!(
            value.thread_id.as_ref().is_none_or(|id| !id.is_empty())
                && value.turn_id.as_ref().is_none_or(|id| !id.is_empty()),
            "empty event scope identity"
        );
        let thread = value.thread_id.as_deref();
        let turn = value.turn_id.as_deref();
        match &value.payload {
            Payload::Ancestry { parent_id, .. } => ensure!(
                !parent_id.is_empty() && thread.is_some() && turn.is_none(),
                "invalid ancestry identity"
            ),
            Payload::Thread { value: fact, .. } => ensure!(
                thread == Some(fact.id.as_ref())
                    && turn.is_none()
                    && fact.source_instance_id.as_ref()
                        == value.position.source_instance_id.as_ref(),
                "thread event scope mismatch"
            ),
            Payload::Turn { value: fact, .. } => ensure!(
                thread == Some(fact.thread_id.as_ref()) && turn == Some(fact.id.as_ref()),
                "turn event scope mismatch"
            ),
            Payload::Measurement {
                value: fact,
                context_conflicts,
                ..
            } => {
                crate::adapters::contract::validate_token_observations(
                    &fact.tokens,
                    &fact.token_unavailable_reasons,
                )?;
                ensure!(
                    thread == fact.thread_id.as_deref()
                        && turn == fact.turn_id.as_deref()
                        && fact.source_instance_id.as_ref()
                            == value.position.source_instance_id.as_ref(),
                    "measurement event scope mismatch"
                );
                ensure!(
                    context_conflicts.len() <= 4
                        && context_conflicts
                            .iter()
                            .enumerate()
                            .all(|(index, field)| !context_conflicts[..index].contains(field)),
                    "invalid measurement context conflict flags"
                );
                ensure!(
                    fact.pricing_context_conflict
                        == context_conflicts
                            .iter()
                            .any(|field| *field != MeasurementContextField::Effort),
                    "measurement pricing conflict flag mismatch"
                );
                ensure!(
                    context_conflicts.iter().all(|field| match field {
                        MeasurementContextField::Model => fact.model.raw.is_none(),
                        MeasurementContextField::Provider => fact.model.provider.is_none(),
                        MeasurementContextField::ApiProvider => fact.model.api_provider.is_none(),
                        MeasurementContextField::Effort => fact.reasoning_effort.is_none(),
                    }),
                    "conflicting measurement context must be unknown"
                );
            }
            Payload::Operation { value: fact, .. } => {
                ensure!(
                    thread == Some(fact.thread_id.as_ref()) && turn == fact.turn_id.as_deref(),
                    "operation event scope mismatch"
                );
                if let Some(work) = &fact.work {
                    work.validate()?;
                }
                fact.validate_matching()?;
            }
            Payload::ContextWindow { tokens, .. } => {
                ensure!(*tokens > 0, "context window must be positive")
            }
            Payload::Lifecycle { native_id, .. } | Payload::Message { native_id, .. } => ensure!(
                native_id.as_ref().is_none_or(|id| !id.is_empty()),
                "empty lifecycle identity"
            ),
            Payload::Item {
                native_id,
                duration,
                started_at_ms,
                completed_at_ms,
                ..
            } => {
                ensure!(
                    native_id.as_ref().is_none_or(|id| !id.is_empty()),
                    "empty item identity"
                );
                ensure!(
                    duration.as_ref().is_none_or(
                        |d| d.nanos < 1_000_000_000 && d.secs <= MAX_SAFE_INTEGER / 1000
                    ),
                    "invalid native duration"
                );
                ensure!(
                    [started_at_ms, completed_at_ms]
                        .into_iter()
                        .all(|v| v.is_none_or(|n| n.unsigned_abs() <= MAX_SAFE_INTEGER)),
                    "unsafe native timestamp"
                );
            }
            Payload::Activity { .. } => {}
            Payload::Review { observation } => {
                ensure!(thread.is_some(), "review observation requires thread");
                observation.validate()?;
            }
        }
        value.time.validate()?;
        Ok(Self(value))
    }
}
impl From<Event> for StoredEvent {
    fn from(value: Event) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests;
