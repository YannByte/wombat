//! Independent share-v1 construction. No read view, locator, absolute time, or alias map.
use crate::timing_dto::*;
use std::collections::BTreeMap;
struct Aliases {
    prefix: String,
    evidence: BTreeMap<String, String>,
    segments: BTreeMap<String, String>,
}
impl Aliases {
    fn new() -> Self {
        Self {
            prefix: uuid::Uuid::new_v4().to_string(),
            evidence: BTreeMap::new(),
            segments: BTreeMap::new(),
        }
    }
    fn evidence(&mut self, value: &str) -> String {
        let n = self.evidence.len() + 1;
        self.evidence
            .entry(value.into())
            .or_insert_with(|| {
                format!(
                    "{}:E-{}-{n}",
                    if value.starts_with("collection:") {
                        "collection"
                    } else if value.starts_with("interval:") {
                        "interval"
                    } else if value.starts_with("measurement:") {
                        "measurement"
                    } else {
                        "event"
                    },
                    self.prefix
                )
            })
            .clone()
    }
    fn segment(&mut self, value: &str) -> String {
        let n = self.segments.len() + 1;
        self.segments
            .entry(value.into())
            .or_insert_with(|| format!("S-{}-{n}", self.prefix))
            .clone()
    }
    fn metric<T: Clone>(&mut self, metric: &Metric<T>) -> Metric<T> {
        Metric {
            value: metric.value.clone(),
            status: metric.status,
            basis: metric.basis,
            evidence_refs: metric
                .evidence_refs
                .iter()
                .map(|id| self.evidence(id))
                .collect(),
        }
    }
    fn distribution(&mut self, d: &Distribution) -> Distribution {
        Distribution {
            samples: self.metric(&d.samples),
            median: self.metric(&d.median),
            p90: self.metric(&d.p90),
        }
    }
    fn input_change(&mut self, c: &InputChange) -> InputChange {
        InputChange {
            method: c.method,
            availability: c.availability.clone(),
            statistics: c.statistics.as_ref().map(|s| {
                let mut value = s.clone();
                value.maximum_increase = self.metric(&s.maximum_increase);
                value.stages = s
                    .stages
                    .iter()
                    .map(|stage| self.input_stage(stage))
                    .collect();
                value.largest_increase = s
                    .largest_increase
                    .as_ref()
                    .map(|stage| self.input_stage(stage));
                value
            }),
        }
    }
    fn input_stage(&mut self, s: &InputChangeStage) -> InputChangeStage {
        InputChangeStage {
            id: self.segment(&s.id),
            first_ref: self.evidence(&s.first_ref),
            last_ref: self.evidence(&s.last_ref),
            ..s.clone()
        }
    }
    fn category(&mut self, c: &Category) -> Category {
        Category {
            candidates: self.metric(&c.candidates),
            closed: self.metric(&c.closed),
            union_ms: self.metric(&c.union_ms),
            sum_ms: self.metric(&c.sum_ms),
        }
    }
    fn repeated_duration(&mut self, d: &RepeatedDuration) -> RepeatedDuration {
        RepeatedDuration {
            known_sum_ms: self.metric(&d.known_sum_ms),
            recorded_count: self.metric(&d.recorded_count),
            calculated_count: self.metric(&d.calculated_count),
            missing_count: self.metric(&d.missing_count),
        }
    }
    fn repeated_metric(&mut self, m: &RepeatedMetric) -> RepeatedMetric {
        RepeatedMetric {
            count: self.metric(&m.count),
            duration: self.repeated_duration(&m.duration),
        }
    }
    fn repeats(&mut self, r: &RepeatedBehavior) -> RepeatedBehavior {
        RepeatedBehavior {
            failure_method: r.failure_method,
            read_method: r.read_method,
            endpoint_method_version: r.endpoint_method_version,
            support: r.support.clone(),
            read_layer: r.read_layer,
            after_failure: self.repeated_metric(&r.after_failure),
            repeated_read: self.repeated_metric(&r.repeated_read),
            same_request_observation_count: self.metric(&r.same_request_observation_count),
            repeated_read_request_count: self.metric(&r.repeated_read_request_count),
            recovery_span_sum_ms: self.metric(&r.recovery_span_sum_ms),
            missing_recovery_span_count: self.metric(&r.missing_recovery_span_count),
            combined_operation_count: self.metric(&r.combined_operation_count),
            combined_union_ms: self.metric(&r.combined_union_ms),
            combined_missing_interval_count: self.metric(&r.combined_missing_interval_count),
            coverage: RepeatCoverage {
                candidate_operations: self.metric(&r.coverage.candidate_operations),
                eligible_commands: self.metric(&r.coverage.eligible_commands),
                missing_identity_records: self.metric(&r.coverage.missing_identity_records),
                excluded_receivers: self.metric(&r.coverage.excluded_receivers),
                missing_matching: self.metric(&r.coverage.missing_matching),
                conflicting_operations: self.metric(&r.coverage.conflicting_operations),
                missing_start: self.metric(&r.coverage.missing_start),
                indeterminate_outcomes: self.metric(&r.coverage.indeterminate_outcomes),
                order_gaps: self.metric(&r.coverage.order_gaps),
                context_boundaries: self.metric(&r.coverage.context_boundaries),
                crossed_context: self.metric(&r.coverage.crossed_context),
                missing_clock_domain: self.metric(&r.coverage.missing_clock_domain),
                source_metadata_gaps: self.metric(&r.coverage.source_metadata_gaps),
                duration_conflicts: self.metric(&r.coverage.duration_conflicts),
                partial: r.coverage.partial,
                reason_codes: r.coverage.reason_codes.clone(),
            },
        }
    }
    fn neighbor(&mut self, n: &Neighbor) -> Neighbor {
        Neighbor {
            measurement_ref: self.evidence(&n.measurement_ref),
            raw_input: self.metric(&n.raw_input),
            ratio: self.metric(&n.ratio),
            distance_ms: self.metric(&n.distance_ms),
        }
    }
}
pub(super) fn project(local: &LocalResponse) -> ShareResponse {
    let mut a = Aliases::new();
    let time = &local.time;
    let coverage = &time.operation_coverage;
    let time = Time {
        repeated_behavior: a.repeats(&time.repeated_behavior),
        operation_coverage: OperationCoverage {
            method_version: coverage.method_version,
            endpoint_method_version: coverage.endpoint_method_version,
            candidate_operations: a.metric(&coverage.candidate_operations),
            paired_operations: a.metric(&coverage.paired_operations),
            identity_gap_records: a.metric(&coverage.identity_gap_records),
            conflicting_operations: a.metric(&coverage.conflicting_operations),
            covered_ms: a.metric(&coverage.covered_ms),
            residual_ms: a.metric(&coverage.residual_ms),
            residual_range_count: a.metric(&coverage.residual_range_count),
            partial: coverage.partial,
            reason_codes: coverage.reason_codes.clone(),
            detail: coverage.detail.clone(),
            detail_limit: coverage.detail_limit,
            residual_ranges: coverage.residual_ranges.clone(),
        },
        timeline: Timeline {
            presentation: time.timeline.presentation,
            detail: time.timeline.detail.clone(),
            entry_count: a.metric(&time.timeline.entry_count),
            track_count: a.metric(&time.timeline.track_count),
            identified_interval_count: a.metric(&time.timeline.identified_interval_count),
            unclassified_gap_count: a.metric(&time.timeline.unclassified_gap_count),
            unlocated_interval_count: a.metric(&time.timeline.unlocated_interval_count),
            outside_window_interval_count: a.metric(&time.timeline.outside_window_interval_count),
            detail_limit: super::intervals::DETAIL_LIMIT,
            tracks: time
                .timeline
                .tracks
                .iter()
                .map(|track| TimelineTrack {
                    interval_alias: a.evidence(&track.interval_alias),
                    category: track.category,
                    start_ms: track.start_ms,
                    end_ms: track.end_ms,
                    clipped: track.clipped,
                    evidence_scope: track.evidence_scope,
                    evidence_refs: track
                        .evidence_refs
                        .iter()
                        .map(|id| a.evidence(id))
                        .collect(),
                })
                .collect(),
            unclassified_gaps: time
                .timeline
                .unclassified_gaps
                .iter()
                .map(|gap| TimelineGap {
                    start_ms: gap.start_ms,
                    end_ms: gap.end_ms,
                    evidence_scope: gap.evidence_scope,
                    evidence_refs: gap.evidence_refs.iter().map(|id| a.evidence(id)).collect(),
                })
                .collect(),
        },
        state: time.state.clone(),
        native_wall_clock_ms: a.metric(&time.native_wall_clock_ms),
        derived_wall_clock_ms: a.metric(&time.derived_wall_clock_ms),
        native_ttft_ms: a.metric(&time.native_ttft_ms),
        first_content_record_delay_ms: a.metric(&time.first_content_record_delay_ms),
        boundary_discrepancy_ms: a.metric(&time.boundary_discrepancy_ms),
        observed_window_ms: a.metric(&time.observed_window_ms),
        command: a.category(&time.command),
        compaction: a.category(&time.compaction),
        reasoning: a.category(&time.reasoning),
        mcp: a.category(&time.mcp),
        intersection_masks_ms: time
            .intersection_masks_ms
            .iter()
            .map(|m| a.metric(m))
            .collect(),
        covered_ms: a.metric(&time.covered_ms),
        unclassified_ms: a.metric(&time.unclassified_ms),
        coverage_ratio: a.metric(&time.coverage_ratio),
        waiting_proxy_ms: a.metric(&time.waiting_proxy_ms),
        strict_response_gap_ms: a.metric(&time.strict_response_gap_ms),
        exploratory_gap_ms: a.metric(&time.exploratory_gap_ms),
    };
    let c = &local.context;
    let context = Context {
        input_change: a.input_change(&c.input_change),
        active_context_occupancy: a.metric(&c.active_context_occupancy),
        compaction_records: a.metric(&c.compaction_records),
        compaction_time_ms: a.metric(&c.compaction_time_ms),
        method: super::context::METHOD.into(),
        quantile_method: super::context::QUANTILE_METHOD.into(),
        candidates: a.metric(&c.candidates),
        conflicting_measurements: a.metric(&c.conflicting_measurements),
        conflicting_window_records: a.metric(&c.conflicting_window_records),
        input: a.distribution(&c.input),
        ratio: a.distribution(&c.ratio),
        segment_count: a.metric(&c.segment_count),
        segments: c
            .segments
            .iter()
            .map(|s| Segment {
                id: a.segment(&s.id),
                candidates: a.metric(&s.candidates),
                non_request_scoped: a.metric(&s.non_request_scoped),
                missing_raw_input: a.metric(&s.missing_raw_input),
                missing_window: a.metric(&s.missing_window),
                invalid_window: a.metric(&s.invalid_window),
                same_record_windows: a.metric(&s.same_record_windows),
                continued_windows: a.metric(&s.continued_windows),
                above_window: a.metric(&s.above_window),
                input: a.distribution(&s.input),
                ratio: a.distribution(&s.ratio),
            })
            .collect(),
        compaction_neighbors: c
            .compaction_neighbors
            .iter()
            .map(|n| CompactionNeighbors {
                evidence_ref: a.evidence(&n.evidence_ref),
                segment_id: a.segment(&n.segment_id),
                before: n.before.as_ref().map(|n| a.neighbor(n)),
                after: n.after.as_ref().map(|n| a.neighbor(n)),
            })
            .collect(),
        detail: c.detail.clone(),
    };
    let w = &local.work;
    let o = &w.outcomes;
    let work = Work {
        outcomes: OutcomeStatistics {
            method: o.method,
            determinate_operations: a.metric(&o.determinate_operations),
            succeeded: a.metric(&o.succeeded),
            failed: a.metric(&o.failed),
            interrupted: a.metric(&o.interrupted),
            rejected: a.metric(&o.rejected),
            nonterminal: a.metric(&o.nonterminal),
            indeterminate: a.metric(&o.indeterminate),
            conflicting: a.metric(&o.conflicting),
            identity_gap_records: a.metric(&o.identity_gap_records),
            unclassified: a.metric(&o.unclassified),
            failure_ratio: a.metric(&o.failure_ratio),
            partial: o.partial,
        },
        operation_candidates: a.metric(&w.operation_candidates),
        closed_operations: a.metric(&w.closed_operations),
        failed_operations: a.metric(&w.failed_operations),
        labelled_command_ms: a.metric(&w.labelled_command_ms),
        file_change_records: a.metric(&w.file_change_records),
        changed_files: a.metric(&w.changed_files),
        added_lines: a.metric(&w.added_lines),
        removed_lines: a.metric(&w.removed_lines),
        message_record_candidates: a.metric(&w.message_record_candidates),
        nonempty_visible_content_records: a.metric(&w.nonempty_visible_content_records),
        unknown_content_records: a.metric(&w.unknown_content_records),
        missing_content_time_records: a.metric(&w.missing_content_time_records),
        user_boundary_records: a.metric(&w.user_boundary_records),
        injected_context_records: a.metric(&w.injected_context_records),
        reasoning_message_records: a.metric(&w.reasoning_message_records),
        compaction_records: a.metric(&w.compaction_records),
        repository_baseline: w.repository_baseline.clone(),
    };
    let c = &local.coverage;
    let coverage = Coverage {
        facts: a.metric(&c.facts),
        bytes: a.metric(&c.bytes),
        metadata: a.metric(&c.metadata),
        event_blocks: a.metric(&c.event_blocks),
        scoped_events: a.metric(&c.scoped_events),
        scoped_measurements: a.metric(&c.scoped_measurements),
        boundary_candidates: a.metric(&c.boundary_candidates),
        lifecycle_candidates: c.lifecycle_candidates.iter().map(|m| a.metric(m)).collect(),
        linked_lifecycles: c.linked_lifecycles.iter().map(|m| a.metric(m)).collect(),
        conflicting_lifecycles: a.metric(&c.conflicting_lifecycles),
        missing_identity_lifecycles: a.metric(&c.missing_identity_lifecycles),
        content_candidates: a.metric(&c.content_candidates),
        domain_count: a.metric(&c.domain_count),
        missing_watermarks: a.metric(&c.missing_watermarks),
        generation_mismatches: a.metric(&c.generation_mismatches),
        incomplete_domains: a.metric(&c.incomplete_domains),
        snapshot_unassigned_total: a.metric(&c.snapshot_unassigned_total),
        thread_unassigned_total: a.metric(&c.thread_unassigned_total),
        source_status: safe_status(&c.source_status),
    };
    let findings = local
        .findings
        .iter()
        .map(|f| Finding {
            code: safe_error(&f.code)
                .unwrap_or("TIMING_DETAIL_UNAVAILABLE")
                .into(),
            kind: f.kind.clone(),
            metric_refs: f
                .metric_refs
                .iter()
                .filter(|s| ["quality", "time", "context", "work"].contains(&s.as_str()))
                .cloned()
                .collect(),
            evidence_refs: f.evidence_refs.iter().map(|id| a.evidence(id)).collect(),
        })
        .collect();
    let start = local.anchors.start_ms.value.map(|_| 0);
    let relative_end = local
        .anchors
        .end_ms
        .value
        .zip(local.anchors.start_ms.value)
        .and_then(|(end, start)| end.checked_sub(start))
        .filter(|value| value.unsigned_abs() <= MAX_SAFE_INTEGER);
    let task_alias = format!("T-{}", a.prefix);
    let turn_alias = format!("R-{}", a.prefix);
    let basis_collections = local
        .evidence
        .collections
        .iter()
        .map(|c| ShareCollection {
            reference: a.evidence(&c.reference),
            kind: c.kind,
            task_alias: task_alias.clone(),
            turn_alias: turn_alias.clone(),
            count: a.metric(&c.count),
            method: c.method.clone(),
        })
        .collect();
    ShareResponse {
        uses: UseTotals {
            method_version: crate::usage_observations::METHOD_VERSION,
            source_coverage: local.uses.totals.source_coverage,
            object_count: a.metric(&local.uses.totals.object_count),
            record_count: a.metric(&local.uses.totals.record_count),
            unbound_target_records: a.metric(&local.uses.totals.unbound_target_records),
            unassigned_skill_records: a.metric(&local.uses.totals.unassigned_skill_records),
            unassigned_mcp_records: a.metric(&local.uses.totals.unassigned_mcp_records),
            coverage: UseCoverage {
                dispatch_gaps: a.metric(&local.uses.totals.coverage.dispatch_gaps),
                identity_gaps: a.metric(&local.uses.totals.coverage.identity_gaps),
                target_gaps: a.metric(&local.uses.totals.coverage.target_gaps),
                time_gaps: a.metric(&local.uses.totals.coverage.time_gaps),
                associated_turn_gaps: a.metric(&local.uses.totals.coverage.associated_turn_gaps),
            },
        },
        basis_collections,
        output_version: OUTPUT_VERSION,
        action: SummaryAction::Summary,
        method_version: METHOD_VERSION.into(),
        profile: ShareProfile::ShareV1,
        privacy: Privacy {
            profile: PrivacyProfile::ShareV1,
            omitted_fields: [
                "read_view",
                "local_scope",
                "local_evidence",
                "absolute_timestamps",
                "paths",
                "native_and_wombat_ids",
                "custom_names",
                "source_bodies",
                "annotation_prose",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            aliases: "fresh_export".into(),
        },
        scope: ShareScope {
            task_alias,
            turn_alias,
            whole_turn: true,
        },
        capabilities: local.capabilities.clone(),
        relative_anchors: Anchors {
            start_ms: Metric {
                value: start,
                status: local.anchors.start_ms.status,
                basis: local.anchors.start_ms.basis,
                evidence_refs: local
                    .anchors
                    .start_ms
                    .evidence_refs
                    .iter()
                    .map(|id| a.evidence(id))
                    .collect(),
            },
            end_ms: Metric {
                value: relative_end,
                status: if relative_end.is_some() {
                    MetricStatus::Derived
                } else {
                    MetricStatus::Unavailable
                },
                basis: local.anchors.end_ms.basis,
                evidence_refs: local
                    .anchors
                    .end_ms
                    .evidence_refs
                    .iter()
                    .map(|id| a.evidence(id))
                    .collect(),
            },
        },
        time,
        context,
        work,
        findings,
        coverage,
        quality: local.quality.clone(),
        freshness: ShareFreshness {
            status: safe_status(&local.freshness.status),
            error_code: local
                .freshness
                .error_code
                .as_deref()
                .and_then(safe_error)
                .map(str::to_owned),
        },
    }
}
fn safe_status(value: &str) -> String {
    match value {
        "complete" | "partial" | "failed" | "not_found" | "cancelled" | "snapshot" | "fixed"
        | "syncing" | "fresh" | "cached" | "updating" | "current" | "stale" => value.into(),
        _ => "unknown".into(),
    }
}
fn safe_error(value: &str) -> Option<&'static str> {
    [
        "INVALID_ARGUMENT",
        "EVIDENCE_UNAVAILABLE",
        "SNAPSHOT_CORRUPT",
        "SOURCE_UNREADABLE",
        "RESOURCE_LIMIT",
        "CANCELLED",
        "TIMING_DETAIL_UNAVAILABLE",
        "TIMING_BOUNDARY_CONFLICT",
        "SOURCE_PARTIAL",
        "NUMERIC_RANGE",
    ]
    .into_iter()
    .find(|code| *code == value)
}
