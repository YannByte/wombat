//! Optional instrumentation around the actual reducer. No alternate counting engine,
//! stored execution history, source bodies, or retained intermediate states.
use super::*;
use crate::adapters::contract::{Operation, Thread};
use sha2::{Digest, Sha256};

pub(super) struct Trace<'a> {
    request: &'a Request,
    records: Vec<UseTraceRecord>,
    total: usize,
    counted: u64,
    reasons: BTreeMap<UseReason, u64>,
    input: Sha256,
    result: Sha256,
    input_recorded: bool,
}
impl<'a> Trace<'a> {
    pub(super) fn new(request: &'a Request) -> Self {
        Self {
            request,
            records: vec![],
            total: 0,
            counted: 0,
            reasons: BTreeMap::new(),
            input: Sha256::new(),
            result: Sha256::new(),
            input_recorded: false,
        }
    }
    pub(super) fn record(
        &mut self,
        op: &Operation,
        thread: &Thread,
        item: Option<&Item>,
        reason: UseReason,
        delta: Option<u64>,
    ) -> Result<()> {
        let r = self.request;
        if r.action != Action::Explain {
            return Ok(());
        }
        let unassigned_kind = if matches!(
            op.kind.as_ref(),
            "mcpTool" | "mcpResource" | "mcpConflict" | "mcpUnclassified"
        ) {
            Kind::Mcp
        } else if usage_observations::read_targets(op).is_some_and(|reads| {
            reads.unbound
                || reads
                    .paths
                    .iter()
                    .any(|path| usage_observations::is_skill_file(path))
        }) {
            Kind::Skill
        } else {
            Kind::Rule
        };
        let selected_item = r
            .item_id
            .as_ref()
            .is_none_or(|id| item.is_some_and(|item| &item.id == id));
        let relevant_kind = item.map_or_else(
            || r.kind.as_ref().is_none_or(|kind| kind == &unassigned_kind),
            |item| r.kind.as_ref().is_none_or(|kind| kind == &item.kind),
        );
        // Bind only selected object inputs and unresolved/conflicting dependencies,
        // rather than invalidating a comparison for an unrelated known object.
        if relevant_kind
            && !self.input_recorded
            && (selected_item || item.is_none() || reason == UseReason::IdentityConflict)
        {
            self.input.update(
                super::super::observations::hash_safe(&(
                    op,
                    &thread.source_instance_id,
                    &thread.project,
                ))?
                .as_bytes(),
            );
            self.input_recorded = true;
        }
        if r.unassigned && item.is_some()
            || r.item_id
                .as_ref()
                .is_some_and(|id| item.is_none_or(|item| &item.id != id))
            || item.is_none() && r.kind.as_ref().is_some_and(|kind| kind != &unassigned_kind)
            || item.is_some_and(|item| {
                r.kind.as_ref().is_some_and(|kind| kind != &item.kind)
                    || r.kinds
                        .as_ref()
                        .is_some_and(|kinds| !kinds.contains(&item.kind))
                    || r.search.as_ref().is_some_and(|search| {
                        !format!("{} {}", item.name, item.path)
                            .to_lowercase()
                            .contains(&search.to_lowercase())
                    })
            })
        {
            return Ok(());
        }
        // A known target outside the requested inventory is still useful when
        // explicitly inspecting unassigned records. Never invent its item ID.
        self.result.update(
            super::super::observations::hash_safe(&(item.map(|i| &i.id), reason, delta))?
                .as_bytes(),
        );
        self.total += 1;
        self.counted += delta.unwrap_or(0);
        *self.reasons.entry(reason).or_default() += 1;
        let offset = r.offset.unwrap_or(0);
        if self.total > offset && self.records.len() < r.limit.unwrap_or(50) {
            self.records.push(UseTraceRecord {
                operation_id: op.id.clone(),
                source_instance_id: thread.source_instance_id.clone(),
                thread_id: thread.id.clone(),
                turn_id: op.turn_id.as_deref().map(str::to_owned),
                item_id: item.map(|item| item.id.clone()),
                timestamp: op.timestamp.clone(),
                event_type: op.kind.to_string(),
                outcome: op.status.to_string(),
                reason,
                count_delta: delta,
                evidence: op.evidence.first().cloned(),
                source_observation_count: op.evidence.len(),
                additional_evidence: op.evidence.iter().skip(1).take(3).cloned().collect(),
            });
        }
        Ok(())
    }
    pub(super) fn begin(&mut self) {
        self.input_recorded = false;
    }
    pub(super) fn finish(
        mut self,
        scope: &Scope,
        view: &View,
        read_complete: Option<bool>,
    ) -> Result<Option<UseExplanation>> {
        if self.request.action != Action::Explain {
            return Ok(None);
        }
        let r = self.request;
        self.input
            .update(super::super::observations::hash_safe(&read_complete)?.as_bytes());
        let mut source_issues = vec![];
        let mut source_issues_total = 0;
        if let Some(snapshot) = &view.snapshot {
            let selected = |id: &str| {
                scope
                    .source_instance_id
                    .as_ref()
                    .is_none_or(|source| source == id)
                    && scope.thread_id.as_ref().is_none_or(|thread| {
                        snapshot.manifest.threads.iter().any(|entry| {
                            &entry.thread.id == thread && entry.thread.source_instance_id == id
                        })
                    })
            };
            for issue in snapshot
                .manifest
                .issues
                .iter()
                .filter(|issue| issue.source_instance_id.as_deref().is_none_or(selected))
            {
                source_issues_total += 1;
                if source_issues.len() < 20 {
                    source_issues.push(UseSourceIssue {
                        code: issue.code.clone(),
                        source_instance_id: issue.source_instance_id.clone(),
                        evidence: issue.evidence.clone(),
                    });
                }
            }
        }
        let baseline = UseReplayBaseline {
            format_version: 1,
            method_version: usage_observations::METHOD_VERSION,
            scope: scope.clone(),
            roots: view.roots.clone(),
            project_roots: view.project_roots.clone(),
            kind: r.kind.clone(),
            item_id: r.item_id.clone(),
            unassigned: r.unassigned,
            input_revision: format!("{:x}", self.input.finalize()),
            configuration_revision: view.observation_versions()?.config_revision.clone(),
            result_revision: format!("{:x}", self.result.finalize()),
            counted: self.counted,
            total_records: self.total,
            reasons: self
                .reasons
                .into_iter()
                .map(|(reason, records)| UseReasonCount { reason, records })
                .collect(),
        };
        let comparison = r.baseline.as_ref().map(|old| {
            let status = if old.scope != baseline.scope
                || old.kind != baseline.kind
                || old.item_id != baseline.item_id
                || old.unassigned != baseline.unassigned
            {
                UseComparisonStatus::ScopeChanged
            } else if old.configuration_revision != baseline.configuration_revision {
                UseComparisonStatus::ConfigurationChanged
            } else if old.input_revision != baseline.input_revision {
                UseComparisonStatus::InputChanged
            } else if old.result_revision != baseline.result_revision
                || old.counted != baseline.counted
                || old.reasons != baseline.reasons
            {
                UseComparisonStatus::Changed
            } else {
                UseComparisonStatus::Unchanged
            };
            let comparable = matches!(
                status,
                UseComparisonStatus::Unchanged | UseComparisonStatus::Changed
            );
            let key = |row: &UseTraceRecord| {
                (
                    row.source_instance_id.clone(),
                    row.thread_id.clone(),
                    row.operation_id.clone(),
                    row.item_id.clone(),
                )
            };
            let old_records = r.baseline_records.as_deref().unwrap_or(&[]);
            let before = old_records
                .iter()
                .map(|row| (key(row), row))
                .collect::<BTreeMap<_, _>>();
            let after = self
                .records
                .iter()
                .map(|row| (key(row), row))
                .collect::<BTreeMap<_, _>>();
            let keys = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
            let complete = comparable
                && old.total_records == old_records.len()
                && baseline.total_records == self.records.len()
                && before.len() == old_records.len()
                && after.len() == self.records.len();
            let changes = if status == UseComparisonStatus::Changed {
                keys.into_iter()
                    .filter(|key| complete || before.contains_key(*key) && after.contains_key(*key))
                    .filter_map(|key| {
                        let a = before.get(key).copied();
                        let b = after.get(key).copied();
                        if a == b {
                            return None;
                        }
                        Some(UseRecordChange {
                            operation_id: key.2.clone(),
                            thread_id: key.1.clone(),
                            item_id: key.3.clone(),
                            before: a.map(|row| row.reason),
                            after: b.map(|row| row.reason),
                            before_delta: a.and_then(|row| row.count_delta),
                            after_delta: b.and_then(|row| row.count_delta),
                        })
                    })
                    .collect()
            } else {
                vec![]
            };
            UseComparison {
                status,
                before: old.counted,
                after: baseline.counted,
                delta: comparable.then(|| baseline.counted as i64 - old.counted as i64),
                changes,
                changes_complete: complete,
            }
        });
        let offset = r.offset.unwrap_or(0);
        let limit = r.limit.unwrap_or(50);
        Ok(Some(UseExplanation {
            source_issues,
            source_issues_total,
            baseline,
            records: self.records,
            page: crate::usage_app_dto::Page {
                offset,
                limit,
                total: self.total,
                next_offset: (offset.saturating_add(limit) < self.total)
                    .then_some(offset.saturating_add(limit)),
            },
            comparison,
        }))
    }
}
