//! Temporary source-ordered replay stores only fields that affect accounting.
//! Safe observations remain complete in the archive and association reduction.
use super::*;
use std::borrow::Cow;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub(in crate::adapters::codex) enum Replay<'a> {
    Thread {
        source: Cow<'a, str>,
        upstream: Cow<'a, str>,
        started: Option<Cow<'a, str>>,
        project: Option<Cow<'a, str>>,
        activity: Option<Cow<'a, str>>,
    },
    Turn {
        thread: Cow<'a, str>,
        upstream: Cow<'a, str>,
        started: Option<Cow<'a, str>>,
        status: Cow<'a, str>,
    },
    Ancestry {
        thread: Cow<'a, str>,
        parent: Cow<'a, str>,
    },
    Measurement {
        value: Cow<'a, Arc<Measurement>>,
        conflicts: Cow<'a, [crate::session_events::MeasurementContextField]>,
        direct: bool,
        cumulative: Option<u64>,
        interval_start: Option<u64>,
        fingerprint: Cow<'a, str>,
    },
    Activity {
        turn: Cow<'a, str>,
        timestamp: Cow<'a, str>,
    },
}
impl<'a> Replay<'a> {
    pub(in crate::adapters::codex) fn from_event(event: &'a Event) -> Option<Self> {
        let optional = |s: Option<&'a str>| s.map(Cow::Borrowed);
        Some(match event.payload() {
            SafePayload::Thread {
                value,
                project_path,
                ..
            } => Self::Thread {
                source: Cow::Borrowed(&value.source_instance_id),
                upstream: Cow::Borrowed(&value.upstream_id),
                started: optional(value.started_at.as_deref()),
                project: optional(project_path.as_deref()),
                activity: optional(value.last_activity_at.as_deref()),
            },
            SafePayload::Turn { value, .. } => Self::Turn {
                thread: Cow::Borrowed(&value.thread_id),
                upstream: Cow::Borrowed(&value.upstream_id),
                started: optional(value.started_at.as_deref()),
                status: Cow::Borrowed(&value.status),
            },
            SafePayload::Ancestry { parent_id, .. } => Self::Ancestry {
                thread: Cow::Borrowed(event.thread_id()?),
                parent: Cow::Borrowed(parent_id),
            },
            SafePayload::Measurement {
                value,
                context_conflicts,
                direct,
                cumulative,
                interval_start,
                fingerprint,
            } => Self::Measurement {
                value: Cow::Borrowed(value),
                conflicts: Cow::Borrowed(context_conflicts),
                direct: *direct,
                cumulative: *cumulative,
                interval_start: *interval_start,
                fingerprint: Cow::Borrowed(fingerprint),
            },
            SafePayload::Operation { value, .. } => Self::Activity {
                turn: Cow::Borrowed(value.turn_id.as_deref()?),
                timestamp: Cow::Borrowed(value.timestamp.as_deref()?),
            },
            _ => return None,
        })
    }
    pub(in crate::adapters::codex) fn apply(self, facts: &mut Facts, report: &mut SourceReport) {
        match self {
            Self::Thread {
                source,
                upstream,
                started,
                project,
                activity,
            } => {
                let id = facts.project_thread(
                    &source,
                    &upstream,
                    started.as_deref(),
                    project.as_deref(),
                );
                if let Some(time) = activity
                    && let Some(thread) = facts.threads.get_mut(&id)
                    && thread
                        .last_activity_at
                        .as_deref()
                        .is_none_or(|old| old < time.as_ref())
                {
                    thread.last_activity_at = Some(time.into_owned());
                }
            }
            Self::Turn {
                thread,
                upstream,
                started,
                status,
            } => {
                facts.project_turn(
                    &thread,
                    &upstream,
                    started.as_deref(),
                    (status != "unknown").then_some(status.as_ref()),
                );
            }
            Self::Ancestry { thread, parent } => {
                facts
                    .parents
                    .insert(thread.into_owned(), parent.into_owned());
            }
            Self::Measurement {
                value,
                conflicts,
                direct,
                cumulative,
                interval_start,
                fingerprint,
            } => {
                let measurement = match value {
                    Cow::Borrowed(value) => Arc::clone(value),
                    Cow::Owned(mut value) => {
                        facts.strings.measurement(Arc::make_mut(&mut value));
                        value
                    }
                };
                let mut candidate = Candidate {
                    measurement,
                    direct,
                    cumulative,
                    interval_start,
                    fingerprint: fingerprint.into_owned(),
                };
                context::apply(facts, &mut candidate, &conflicts, report);
                facts.project_measurement(candidate, report);
            }
            Self::Activity { turn, timestamp } => {
                facts.observe_operation_activity(Some(&turn), Some(&timestamp));
            }
        }
    }
}
