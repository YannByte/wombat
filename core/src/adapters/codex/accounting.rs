//! Direct and cumulative accounting reconciliation preserves missing and conflicting facts.
use super::*;
impl Facts {
    pub(super) fn measurement(
        &mut self,
        mut candidate: Candidate,
        context_conflicts: &[crate::session_events::MeasurementContextField],
        report: &mut SourceReport,
    ) {
        self.strings
            .measurement(Arc::make_mut(&mut candidate.measurement));
        timing::measurement(self, &candidate, context_conflicts, report);
    }
    pub(super) fn project_measurement(&mut self, candidate: Candidate, report: &mut SourceReport) {
        self.dirty_measurements
            .insert(candidate.measurement.id.clone());
        if let Some(existing) = self.measurements.get_mut(&candidate.measurement.id) {
            existing.direct |= candidate.direct;
            existing.cumulative = existing.cumulative.or(candidate.cumulative);
            existing.interval_start = existing.interval_start.or(candidate.interval_start);
            let incoming = Arc::unwrap_or_clone(candidate.measurement);
            let old = Arc::make_mut(&mut existing.measurement);
            let prefix = old.id.clone();
            let conflicts = &mut self.measurement_conflicts;
            let token_conflict = merge_token_observations(
                &mut old.tokens,
                &mut old.token_unavailable_reasons,
                &incoming.tokens,
                &incoming.token_unavailable_reasons,
            );
            if token_conflict {
                issue(
                    report,
                    "measurementConflict",
                    "同一用量身份出现不同计数",
                    incoming.evidence.first().cloned(),
                );
            }
            if merge_optional(
                &mut old.turn_id,
                &incoming.turn_id,
                conflicts,
                &format!("{prefix}:turn"),
            ) {
                issue(
                    report,
                    "ownershipConflict",
                    "同一用量的轮次归属冲突",
                    incoming.evidence.first().cloned(),
                );
            }
            let mut model_conflict = merge_optional(
                &mut old.model.raw,
                &incoming.model.raw,
                conflicts,
                &format!("{prefix}:model"),
            );
            model_conflict |= merge_optional(
                &mut old.model.provider,
                &incoming.model.provider,
                conflicts,
                &format!("{prefix}:provider"),
            );
            model_conflict |= merge_optional(
                &mut old.model.api_provider,
                &incoming.model.api_provider,
                conflicts,
                &format!("{prefix}:apiProvider"),
            );
            old.pricing_context_conflict |= incoming.pricing_context_conflict || model_conflict;
            if model_conflict {
                issue(
                    report,
                    "modelConflict",
                    "同一用量的模型冲突",
                    incoming.evidence.first().cloned(),
                );
            }
            if merge_optional(
                &mut old.reasoning_effort,
                &incoming.reasoning_effort,
                conflicts,
                &format!("{prefix}:effort"),
            ) {
                issue(
                    report,
                    "contextConflict",
                    "同一用量的推理强度冲突",
                    incoming.evidence.first().cloned(),
                );
            }
            if candidate.direct {
                old.request_scoped = true;
                old.grain = "response".into();
            }
            if incoming
                .timestamp
                .as_ref()
                .is_some_and(|new| old.timestamp.as_ref().is_none_or(|current| new < current))
            {
                old.timestamp = incoming.timestamp;
                old.time_precision = incoming.time_precision;
            }
            for evidence in incoming.evidence {
                if !old.evidence.contains(&evidence) {
                    old.evidence.push(evidence);
                }
            }
        } else {
            self.measurements
                .insert(candidate.measurement.id.clone(), candidate);
        }
    }
    pub(super) fn inherited_measurements(
        &self,
        forest: &session_relations::SessionRelations<'_>,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<BTreeSet<&str>> {
        // Only byte-identical native counter events can be inherited. Direct response
        // records retain their explicit owner, even when their counts match an ancestor.
        let check = crate::operation_association::check;
        check(cancelled)?;
        if self.parents.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut entries = Vec::new();
        for (id, candidate) in &self.measurements {
            check(cancelled)?;
            if let Some(thread) = candidate.measurement.thread_id.as_deref() {
                entries.push((candidate.fingerprint.as_str(), thread, id.as_str()));
            }
        }
        let mut remove = BTreeSet::new();
        for (id, _) in forest.replays_cancellable(entries.into_iter(), cancelled)? {
            check(cancelled)?;
            if !self.measurements[id].direct {
                remove.insert(id);
            }
        }
        check(cancelled)?;
        Ok(remove)
    }
    pub(super) fn direct_exclusions(
        &self,
        inherited: &BTreeSet<&str>,
        report: &mut SourceReport,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<BTreeSet<&str>> {
        let check = crate::operation_association::check;
        check(cancelled)?;
        let selected = || {
            self.measurements
                .iter()
                .filter(|(id, _)| !inherited.contains(id.as_str()))
        };
        let mut direct_only = true;
        for candidate in selected().map(|(_, candidate)| candidate) {
            check(cancelled)?;
            if !candidate.direct {
                direct_only = false;
                break;
            }
        }
        check(cancelled)?;
        if direct_only {
            return Ok(BTreeSet::new());
        }
        // Build coverage once per owner. Scanning every direct response for every
        // legacy counter is quadratic across unrelated historical conversations.
        let mut direct = BTreeMap::<Arc<str>, Vec<(u64, u64)>>::new();
        let mut unbounded = BTreeMap::<Option<Arc<str>>, BTreeSet<Option<Arc<str>>>>::new();
        for candidate in selected().map(|(_, candidate)| candidate) {
            check(cancelled)?;
            if !candidate.direct {
                continue;
            }
            if let (Some(thread), Some(start), Some(end)) = (
                &candidate.measurement.thread_id,
                candidate.interval_start,
                candidate.cumulative,
            ) {
                direct.entry(thread.clone()).or_default().push((start, end));
            }
            if candidate.interval_start.is_none() || candidate.cumulative.is_none() {
                unbounded
                    .entry(candidate.measurement.thread_id.clone())
                    .or_default()
                    .insert(candidate.measurement.turn_id.clone());
            }
        }
        check(cancelled)?;
        for ranges in direct.values_mut() {
            check(cancelled)?;
            ranges.sort_unstable();
            check(cancelled)?;
            let mut merged: Vec<(u64, u64)> = Vec::new();
            for &(start, end) in ranges.iter() {
                check(cancelled)?;
                if let Some(last) = merged.last_mut()
                    && start <= last.1
                {
                    last.1 = last.1.max(end);
                } else {
                    merged.push((start, end));
                }
            }
            *ranges = merged;
        }
        let mut remove = BTreeSet::new();
        for (id, candidate) in selected() {
            check(cancelled)?;
            if candidate.direct {
                continue;
            }
            if unbounded
                .get(&candidate.measurement.thread_id)
                .is_some_and(|turns| {
                    candidate.measurement.turn_id.is_none()
                        || turns.contains(&None)
                        || turns.contains(&candidate.measurement.turn_id)
                })
            {
                issue(
                    report,
                    "usageCoverageUnknown",
                    "同轮存在逐响应用量，未叠加缺少覆盖依据的旧累计记录",
                    candidate.measurement.evidence.first().cloned(),
                );
                remove.insert(id.as_str());
                continue;
            }
            let (Some(thread), Some(start), Some(end)) = (
                &candidate.measurement.thread_id,
                candidate.interval_start,
                candidate.cumulative,
            ) else {
                continue;
            };
            let ranges = direct.get(thread).map(Vec::as_slice).unwrap_or_default();
            let next = ranges.get(ranges.partition_point(|(_, to)| *to <= start));
            let overlaps = next.is_some_and(|(from, _)| *from < end);
            if next.is_some_and(|(from, to)| *from <= start && *to >= end) && end > start {
                remove.insert(id.as_str());
            } else if overlaps {
                issue(
                    report,
                    "usageOverlap",
                    "逐响应记录与累计区间部分重叠；该累计记录未重复计入",
                    candidate.measurement.evidence.first().cloned(),
                );
                remove.insert(id.as_str());
            }
        }
        check(cancelled)?;
        Ok(remove)
    }
}
