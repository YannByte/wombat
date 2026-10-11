//! Physical and native identities, thread/turn facts and operation attribution.
use super::*;
mod association;
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Facts {
    pub(super) watermarks: BTreeMap<String, SourceWatermark>,
    #[serde(skip)]
    pub(super) strings: super::super::shared_strings::FactStrings,
    #[serde(skip)]
    pub(super) event_strings: crate::session_events::EventStrings,
    pub(super) threads: BTreeMap<String, Thread>,
    pub(super) turns: BTreeMap<String, Turn>,
    pub(super) measurements: BTreeMap<String, Candidate>,
    pub(super) operations: BTreeMap<String, Arc<Operation>>,
    #[serde(skip)]
    pub(super) event_context: Option<timing::Context>,
    pub(super) title_observations:
        BTreeMap<String, crate::session_events::title_observations::TitleObservation>,
    #[serde(skip)]
    pub(super) retained_collection_times: BTreeMap<Arc<str>, String>,
    #[serde(skip)]
    pub(super) streaming: bool,
    #[serde(skip)]
    pub(super) pending_events: Vec<Arc<crate::session_events::Event>>,
    pub(super) events: BTreeMap<Arc<str>, Arc<crate::session_events::Event>>,
    #[serde(skip)]
    pub(super) dirty_events: BTreeSet<Arc<str>>,
    #[serde(skip)]
    pub(super) full_event_write: bool,
    #[serde(skip)]
    pub(super) dirty_operations: BTreeSet<String>,
    #[serde(skip)]
    pub(super) operations_pending: bool,
    #[serde(skip)]
    pub(super) association_groups: Option<BTreeMap<String, Vec<association::Prepared>>>,
    #[serde(skip)]
    pub(super) retired_operations: BTreeSet<String>,
    #[serde(skip)]
    pub(super) operation_aliases: BTreeMap<String, Arc<[crate::operation_association::OwnedAlias]>>,
    #[serde(skip)]
    pub(super) dirty_measurements: BTreeSet<String>,
    #[serde(skip)]
    pub(super) dirty_aliases: BTreeSet<String>,
    pub(super) aliases: HashMap<String, String>,
    pub(super) parents: HashMap<String, String>,
    pub(super) measurement_conflicts: BTreeSet<String>,
    pub(super) projects: BTreeMap<String, BTreeSet<String>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Candidate {
    pub(super) measurement: Arc<Measurement>,
    pub(super) direct: bool,
    pub(super) cumulative: Option<u64>,
    pub(super) interval_start: Option<u64>,
    pub(super) fingerprint: String,
}

impl Facts {
    pub(super) fn fork_derived(&self, include_facts: bool) -> Self {
        Self {
            watermarks: self.watermarks.clone(),
            title_observations: self.title_observations.clone(),
            threads: self.threads.clone(),
            turns: self.turns.clone(),
            measurements: if include_facts {
                self.measurements.clone()
            } else {
                BTreeMap::new()
            },
            operations: if include_facts {
                self.operations.clone()
            } else {
                BTreeMap::new()
            },
            operation_aliases: if include_facts {
                self.operation_aliases.clone()
            } else {
                BTreeMap::new()
            },
            parents: self.parents.clone(),
            measurement_conflicts: self.measurement_conflicts.clone(),
            projects: self.projects.clone(),
            ..Self::default()
        }
    }
    pub(super) fn project_thread(
        &mut self,
        source_id: &str,
        upstream: &str,
        timestamp: Option<&str>,
        cwd: Option<&str>,
    ) -> String {
        let id = thread_identity(source_id, upstream);
        if let Some(cwd) = cwd {
            self.projects
                .entry(id.clone())
                .or_default()
                .insert(cwd.into());
        }
        let thread = self.threads.entry(id.clone()).or_insert_with(|| Thread {
            id: id.clone(),
            agent_kind: "codex".into(),
            source_instance_id: source_id.into(),
            upstream_id: upstream.into(),
            title: None,
            project: cwd.map(safe_text),
            started_at: timestamp.map(str::to_owned),
            last_activity_at: timestamp.map(str::to_owned),
        });
        if let Some(time) = timestamp {
            if thread.started_at.as_deref().is_none_or(|old| old > time) {
                thread.started_at = Some(time.into());
            }
            if thread
                .last_activity_at
                .as_deref()
                .is_none_or(|old| old < time)
            {
                thread.last_activity_at = Some(time.into());
            }
        }
        if thread.project.is_none() {
            thread.project = cwd.map(safe_text);
        }
        id
    }
    pub(super) fn project_turn(
        &mut self,
        thread: &str,
        upstream: &str,
        timestamp: Option<&str>,
        status: Option<&str>,
    ) -> String {
        let id = turn_identity(thread, upstream);
        let turn = self.turns.entry(id.clone()).or_insert_with(|| Turn {
            id: id.clone(),
            thread_id: thread.into(),
            upstream_id: upstream.into(),
            ordinal: 0,
            started_at: timestamp.map(str::to_owned),
            ended_at: None,
            last_activity_at: timestamp.map(str::to_owned),
            status: "unknown".into(),
        });
        if let Some(time) = timestamp
            && turn.started_at.as_deref().is_none_or(|old| old > time)
        {
            turn.started_at = Some(time.into());
        }
        if let Some(time) = timestamp
            && turn
                .last_activity_at
                .as_deref()
                .is_none_or(|old| old < time)
        {
            turn.last_activity_at = Some(time.into());
        }
        if let Some(status) = status {
            if status != "running" || turn.status == "unknown" {
                turn.status = status.into();
            }
            if matches!(status, "completed" | "interrupted" | "failed") {
                turn.ended_at = timestamp.map(str::to_owned);
            }
        }
        id
    }
    pub(super) fn operation(&mut self, mut operation: Operation, report: &mut SourceReport) {
        self.strings.operation(&mut operation);
        timing::operation(self, &operation, report);
    }
    /// Full replay uses the same canonical component reduction as scoped archive
    /// reads. Retired identities require atomic replacement.
    pub(super) fn resolve_operations(
        &mut self,
        report: &mut SourceReport,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<()> {
        let check = crate::operation_association::check;
        check(cancelled)?;
        if !self.operations_pending {
            return Ok(());
        }
        let prepared = Self::prepare_associations(
            self.events.values().map(AsRef::as_ref),
            &mut self.strings,
            report,
            cancelled,
        )?;
        self.association_groups = Some(BTreeMap::from([(String::new(), prepared)]));
        self.publish_associations(report, cancelled)
    }
    pub(super) fn observe_operation(&mut self, operation: &Operation) {
        self.observe_operation_activity(
            operation.turn_id.as_deref(),
            operation.timestamp.as_deref(),
        );
    }
    pub(super) fn observe_operation_activity(&mut self, turn: Option<&str>, time: Option<&str>) {
        if let Some(turn) = turn.and_then(|id| self.turns.get_mut(id))
            && let Some(time) = time
            && turn
                .last_activity_at
                .as_deref()
                .is_none_or(|old| old < time)
        {
            turn.last_activity_at = Some(time.to_owned());
        }
    }
}
pub(super) fn merge_optional<T: Clone + Eq>(
    current: &mut Option<T>,
    new: &Option<T>,
    conflicts: &mut BTreeSet<String>,
    key: &str,
) -> bool {
    if conflicts.contains(key) {
        *current = None;
        return false;
    }
    if current
        .as_ref()
        .zip(new.as_ref())
        .is_some_and(|(a, b)| a != b)
    {
        *current = None;
        conflicts.insert(key.into());
        return true;
    }
    if current.is_none() {
        *current = new.clone();
    }
    false
}
