//! One fixed turn, bounded storage reads, and protocol projection. No ledger scans.
use super::{analysis, mapping as m, share, work};
use crate::session_events::{Event, Payload};
use crate::{
    dto::operation_error,
    timing_dto::*,
    usage_store::{
        self, EventCursor, EventReadBudget, EventTarget, Snapshot,
        timing_evidence::{TimingReadBudget, TurnTarget},
    },
};
use anyhow::Result;
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub const MAX_SUMMARY_BYTES: usize = 256 * 1024;
const MAX_CURSOR_BYTES: usize = 8 * 1024;
const MAX_LOCATOR_BYTES: usize = 4096;
mod navigation;
mod repeats;
mod uses;

pub fn validate(request: &Request) -> Result<()> {
    let (thread, turn, snapshot, roots, scope) = match request {
        Request::Capabilities { .. } => return Ok(()),
        Request::Summary {
            thread_id,
            turn_id,
            snapshot_id,
            roots,
            scope,
            mode,
            ..
        } => {
            if snapshot_id.is_some() && *mode == Mode::Fresh {
                return invalid();
            }
            (thread_id, turn_id, snapshot_id.as_deref(), roots, scope)
        }
        Request::Evidence {
            thread_id,
            turn_id,
            snapshot_id,
            roots,
            scope,
            cursor,
            limit,
            privacy_profile,
            collection,
            object_ref,
        } => {
            if *privacy_profile != PrivacyProfile::Local
                || *limit == 0
                || *limit > 200
                || cursor
                    .as_ref()
                    .is_some_and(|c| c.token.len() > MAX_CURSOR_BYTES)
                || (object_ref.is_some() && *collection != EvidenceSet::UseRecords)
                || object_ref.as_ref().is_some_and(|id| {
                    !id.starts_with("use:")
                        || id.len() != 68
                        || !id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit)
                })
            {
                return invalid();
            }
            (thread_id, turn_id, Some(snapshot_id.as_str()), roots, scope)
        }
    };
    if [Some(thread.as_str()), Some(turn.as_str()), snapshot]
        .into_iter()
        .flatten()
        .any(|id| id.is_empty() || id.len() > MAX_LOCATOR_BYTES)
        || roots.len() > 128
        || roots
            .iter()
            .any(|root| root.is_empty() || root.len() > MAX_LOCATOR_BYTES)
        || scope.as_ref().is_some_and(|scope| {
            scope
                .agent_kind
                .as_deref()
                .is_some_and(|agent| agent != "codex")
                || scope
                    .source_instance_id
                    .as_ref()
                    .is_some_and(|id| id.is_empty() || id.len() > MAX_LOCATOR_BYTES)
        })
    {
        return invalid();
    }
    Ok(())
}
fn invalid<T>() -> Result<T> {
    Err(operation_error(
        "INVALID_ARGUMENT",
        "Invalid timing request",
    ))
}
fn check(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(operation_error("CANCELLED", "Timing query cancelled"))
    } else {
        Ok(())
    }
}
pub fn capabilities(profile: PrivacyProfile) -> Result<Response> {
    Ok(Response::Capabilities(CapabilitiesResponse {
        output_version: OUTPUT_VERSION,
        action: CapabilitiesAction::Capabilities,
        method_version: METHOD_VERSION.into(),
        profile,
        capabilities: m::capabilities(),
    }))
}
/// Disk entry uses only the committed snapshot. Live synchronization is a host path.
pub fn dispatch(request: Request) -> Result<Response> {
    dispatch_impl(request).map_err(safe_error)
}
fn dispatch_impl(request: Request) -> Result<Response> {
    validate(&request)?;
    if let Request::Capabilities { privacy_profile } = request {
        return capabilities(privacy_profile);
    }
    let snapshot_id = match &request {
        Request::Summary {
            snapshot_id, mode, ..
        } => {
            if *mode == Mode::Fresh {
                return invalid();
            }
            snapshot_id.as_deref()
        }
        Request::Evidence { snapshot_id, .. } => Some(snapshot_id.as_str()),
        _ => unreachable!(),
    };
    let snapshot = usage_store::load(snapshot_id)?;
    query_on_snapshot(
        &snapshot,
        &request,
        QueryFreshness {
            status: "snapshot".into(),
            ..Default::default()
        },
        &AtomicBool::new(false),
    )
}

pub fn query_on_snapshot(
    snapshot: &Snapshot,
    request: &Request,
    freshness: QueryFreshness,
    cancelled: &AtomicBool,
) -> Result<Response> {
    query_impl(snapshot, request, freshness, cancelled).map_err(safe_error)
}
fn query_impl(
    snapshot: &Snapshot,
    request: &Request,
    freshness: QueryFreshness,
    cancelled: &AtomicBool,
) -> Result<Response> {
    validate(request)?;
    check(cancelled)?;
    let (thread_id, turn_id, snapshot_id, scope, roots, profile) = match request {
        Request::Capabilities { privacy_profile } => return capabilities(*privacy_profile),
        Request::Summary {
            thread_id,
            turn_id,
            snapshot_id,
            scope,
            roots,
            privacy_profile,
            ..
        } => (
            thread_id,
            turn_id,
            snapshot_id.as_deref(),
            scope,
            roots,
            *privacy_profile,
        ),
        Request::Evidence {
            thread_id,
            turn_id,
            snapshot_id,
            scope,
            roots,
            privacy_profile,
            ..
        } => (
            thread_id,
            turn_id,
            Some(snapshot_id.as_str()),
            scope,
            roots,
            *privacy_profile,
        ),
    };
    if snapshot_id.is_some_and(|id| id != snapshot.manifest.snapshot_ref.snapshot_id) {
        return Err(operation_error(
            "EVIDENCE_UNAVAILABLE",
            "Timing read identity is unavailable",
        ));
    }
    if snapshot.manifest.threads.len() > 100_000 || snapshot.manifest.sources.len() > 100_000 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Timing owner metadata exceeds budget",
        ));
    }
    let mut thread = None;
    for entry in &snapshot.manifest.threads {
        check(cancelled)?;
        if entry.thread.id == *thread_id && thread.replace(&entry.thread).is_some() {
            return Err(operation_error(
                "SNAPSHOT_CORRUPT",
                "Ambiguous timing task identity",
            ));
        }
    }
    let thread = thread.ok_or_else(|| {
        operation_error(
            "INVALID_ARGUMENT",
            "Timing task does not belong to this read view",
        )
    })?;
    if thread.agent_kind != "codex"
        || scope.as_ref().is_some_and(|scope| {
            scope
                .source_instance_id
                .as_deref()
                .is_some_and(|id| id != thread.source_instance_id)
        })
    {
        return invalid();
    }
    let source = snapshot
        .manifest
        .sources
        .iter()
        .find(|source| source.source.id == thread.source_instance_id);
    if !roots.is_empty() {
        let source = source.ok_or_else(|| {
            operation_error("INVALID_ARGUMENT", "Timing source scope unavailable")
        })?;
        let path = crate::absolute(&source.source.root)?;
        if !roots
            .iter()
            .map(crate::absolute)
            .collect::<Result<Vec<_>>>()?
            .iter()
            .any(|root| path.starts_with(root))
        {
            return invalid();
        }
    }
    let local_scope = LocalScope {
        source_instance_id: thread.source_instance_id.clone(),
        thread_id: thread_id.clone(),
        turn_id: turn_id.clone(),
        agent_kind: thread.agent_kind.clone(),
        whole_turn: true,
    };
    let target = TurnTarget {
        source: &thread.source_instance_id,
        thread: thread_id,
        turn: turn_id,
    };
    let native = snapshot.timing_native_boundary(target, cancelled)?;
    let cache_key = (profile == PrivacyProfile::Local
        && matches!(request, Request::Summary { .. }))
    .then(|| serde_json::to_string(&(METHOD_VERSION, request)))
    .transpose()?;
    if let Some(key) = &cache_key {
        let hit = snapshot
            .timing_cache
            .lock()
            .map_err(|_| operation_error("CORE_ERROR", "Timing cache unavailable"))?
            .get(key);
        if let Some(mut hit) = hit {
            check(cancelled)?;
            hit.freshness = freshness;
            if !fits(&hit)? {
                return Err(operation_error(
                    "RESOURCE_LIMIT",
                    "Timing summary exceeds output budget",
                ));
            }
            check(cancelled)?;
            return Ok(Response::Local(Box::new(hit)));
        }
    }
    if let Request::Evidence { collection, .. } = request
        && *collection != EvidenceSet::TurnEvents
    {
        return uses::page(snapshot, target, local_scope, request, cancelled);
    }
    if let Request::Evidence { cursor, limit, .. } = request {
        let cursor = cursor.as_ref().map(decode_cursor).transpose()?;
        let page = snapshot.event_page(
            &EventTarget::turn(thread_id, turn_id),
            *limit,
            cursor.as_ref(),
            EventReadBudget::default(),
            cancelled,
        )?;
        let rows = page
            .events
            .iter()
            .map(|event| {
                check(cancelled)?;
                if event.position().source_instance_id.as_ref() != target.source {
                    return Err(operation_error(
                        "SNAPSHOT_CORRUPT",
                        "Timing evidence source mismatch",
                    ));
                }
                evidence_row(event)
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(Response::Evidence(EvidenceResponse {
            collection: EventPageKind::TurnEvents,
            output_version: OUTPUT_VERSION,
            action: EvidenceAction::Evidence,
            method_version: METHOD_VERSION.into(),
            profile: LocalProfile::Local,
            snapshot_id: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
            scope: local_scope,
            total: m::observed(page.total, &[]),
            rows,
            next_cursor: page.next_cursor.map(encode_cursor).transpose()?,
        }));
    }
    let evidence = match snapshot.timing_evidence(target, TimingReadBudget::default(), cancelled) {
        Ok(evidence) => Some(evidence),
        Err(error)
            if error
                .downcast_ref::<crate::dto::OperationError>()
                .is_some_and(|error| error.code == "RESOURCE_LIMIT") =>
        {
            None
        }
        Err(error) => return Err(error),
    };
    let mut events = Vec::new();
    if let Some(e) = &evidence {
        events.extend(e.events.iter().cloned());
        events.extend(e.controls.iter().cloned());
    }
    let mut a = analysis::analyze_cancellable(
        analysis::AnalyzeInput {
            source: target.source,
            thread: thread_id,
            turn: turn_id,
            measurements: evidence.as_ref().map_or(&[], |e| e.measurements.as_slice()),
            events: &events,
            budget: analysis::Budget {
                events: 100_000,
                measurements: 100_000,
                lifecycle_records: 100_000,
            },
        },
        cancelled,
    )?;
    let fallback = evidence.is_none().then_some(Basis::ResourceLimit);
    if fallback.is_some() {
        a.context = None;
        if let Some(native) = native {
            a.state = native.state;
            a.native_wall_clock_ms = native.native_wall_clock_ms;
            a.native_ttft_ms = native.native_ttft_ms;
            if native.identity_conflict
                || native.state_conflict
                || native.boundary_conflict
                || native.native_duration_conflict
                || native.native_ttft_conflict
            {
                a.issues.push(analysis::Issue::BoundaryConflict);
            }
            if native.clock_domain_limit {
                a.issues.push(analysis::Issue::ResourceLimit);
            }
        }
    }
    let refs: Vec<_> = evidence.as_ref().map_or_else(Vec::new, |e| {
        e.events
            .iter()
            .take(8)
            .map(|event| format!("event:{}", event.id()))
            .collect()
    });
    let turn_refs = if evidence.is_some() {
        vec![
            "collection:turn_events".into(),
            "collection:source_controls".into(),
        ]
    } else if native.is_some() {
        vec!["collection:native_boundary_index".into()]
    } else {
        vec![]
    };
    let context_refs = if evidence.is_some() {
        vec![
            "collection:canonical_measurements".into(),
            "collection:turn_events".into(),
            "collection:source_controls".into(),
        ]
    } else {
        vec![]
    };
    let coverage = coverage(
        &a,
        evidence.as_ref(),
        source.map(|source| source.status.as_str()),
        fallback,
    );
    let mut quality = Quality {
        partial: fallback.is_some()
            || a.coverage.partial
            || coverage.incomplete_domains.value.is_none_or(|n| n > 0)
            || source.is_none_or(|source| source.status != "complete"),
        running: a.state == analysis::State::Running,
        censored: a.state == analysis::State::Running,
        reason_codes: Vec::new(),
        fact_limit: 100_000,
        summary_limit_bytes: MAX_SUMMARY_BYTES,
    };
    if fallback.is_some() {
        quality.reason_codes.push(Basis::ResourceLimit);
    }
    if coverage.incomplete_domains.value.is_some_and(|n| n > 0)
        || source.is_none_or(|source| source.status != "complete")
    {
        quality.reason_codes.push(Basis::SourcePartial);
    }
    if a.start.is_none() || a.end.is_none() {
        quality.partial = true;
        quality.reason_codes.push(Basis::MissingTime);
    }
    if a.issues
        .iter()
        .any(|issue| matches!(issue, analysis::Issue::BoundaryConflict))
    {
        quality.partial = true;
        quality.reason_codes.push(Basis::BoundaryConflict);
    }
    if a.issues.iter().any(|issue| {
        matches!(
            issue,
            analysis::Issue::UnknownContent(_)
                | analysis::Issue::MissingContentTime(_)
                | analysis::Issue::ContentConflict(_)
                | analysis::Issue::UnmatchedContentDomain(_)
        )
    }) {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::SourcePartial) {
            quality.reason_codes.push(Basis::SourcePartial);
        }
    }
    if a.native_wall_clock_ms.is_some_and(|n| n > MAX_SAFE_INTEGER)
        || a.native_ttft_ms.is_some_and(|n| n > MAX_SAFE_INTEGER)
        || a.derived_wall_clock_ms
            .is_some_and(|n| n > MAX_SAFE_INTEGER)
        || a.boundary_delta_ms
            .is_some_and(|n| n.unsigned_abs() > u128::from(MAX_SAFE_INTEGER))
        || a.context.as_ref().is_some_and(|c| {
            c.input
                .median
                .into_iter()
                .chain(c.input.p90)
                .any(|n| !n.is_finite() || n > MAX_SAFE_INTEGER as f64)
        })
    {
        quality.partial = true;
        quality.reason_codes.push(Basis::NumericRange);
    }
    if a.issues.iter().any(|issue| {
        matches!(
            issue,
            analysis::Issue::MissingItemIdentity(_) | analysis::Issue::IdentityConflict(_)
        )
    }) {
        quality.partial = true;
        quality.reason_codes.push(Basis::MissingIdentity);
    }
    if a.issues
        .iter()
        .any(|issue| matches!(issue, analysis::Issue::TargetConflict(_)))
    {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::TargetConflict) {
            quality.reason_codes.push(Basis::TargetConflict);
        }
    }
    if a.issues
        .iter()
        .any(|issue| matches!(issue, analysis::Issue::OutcomeConflict(_)))
    {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::OutcomeConflict) {
            quality.reason_codes.push(Basis::OutcomeConflict);
        }
    }
    if a.issues.iter().any(|issue| {
        matches!(
            issue,
            analysis::Issue::MissingItemTime(_) | analysis::Issue::UnmatchedClockDomain(_)
        )
    }) || a.intervals.issues.iter().any(|issue| {
        matches!(
            issue,
            super::intervals::Issue::Open(_)
                | super::intervals::Issue::Reversed(_)
                | super::intervals::Issue::Conflict(_)
        )
    }) {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::MissingTime) {
            quality.reason_codes.push(Basis::MissingTime);
        }
    }
    if a.issues
        .iter()
        .any(|issue| matches!(issue, analysis::Issue::SourceGap(_)))
    {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::SourcePartial) {
            quality.reason_codes.push(Basis::SourcePartial);
        }
    }
    let context = m::context(&a, &context_refs, fallback);
    let object_uses = uses::summary(snapshot, target, evidence.as_ref(), profile, cancelled)?;
    for (gap, basis) in [
        (
            object_uses.detail.reason == Basis::ResourceLimit,
            Basis::ResourceLimit,
        ),
        (
            object_uses
                .totals
                .coverage
                .dispatch_gaps
                .value
                .is_some_and(|n| n > 0),
            Basis::DispatchNotProven,
        ),
        (
            object_uses
                .totals
                .coverage
                .identity_gaps
                .value
                .is_some_and(|n| n > 0),
            Basis::MissingIdentity,
        ),
        (
            object_uses
                .totals
                .coverage
                .target_gaps
                .value
                .is_some_and(|n| n > 0),
            Basis::MissingTarget,
        ),
        (
            object_uses
                .totals
                .coverage
                .time_gaps
                .value
                .is_some_and(|n| n > 0),
            Basis::MissingTime,
        ),
        (
            object_uses
                .totals
                .unassigned_skill_records
                .value
                .is_none_or(|n| n > 0)
                || object_uses
                    .totals
                    .unassigned_mcp_records
                    .value
                    .is_none_or(|n| n > 0),
            Basis::MissingTurn,
        ),
    ] {
        if gap {
            quality.partial = true;
            if !quality.reason_codes.contains(&basis) {
                quality.reason_codes.push(basis);
            }
        }
    }
    let mut time = m::time(&a, &turn_refs, fallback, native);
    if quality.reason_codes.contains(&Basis::SourcePartial) {
        time.repeated_behavior.coverage.partial = true;
        if !time
            .repeated_behavior
            .coverage
            .reason_codes
            .contains(&RepeatCoverageReason::SourcePartial)
        {
            time.repeated_behavior
                .coverage
                .reason_codes
                .push(RepeatCoverageReason::SourcePartial);
        }
        if !matches!(time.repeated_behavior.support.support, Support::Unavailable) {
            time.repeated_behavior.support.support = Support::Partial;
        }
        time.operation_coverage.partial = true;
        if !time
            .operation_coverage
            .reason_codes
            .iter()
            .any(|reason| matches!(reason, OperationCoverageReason::SourcePartial))
        {
            time.operation_coverage
                .reason_codes
                .push(OperationCoverageReason::SourcePartial);
        }
    }
    if time
        .repeated_behavior
        .coverage
        .reason_codes
        .contains(&RepeatCoverageReason::ResourceLimit)
    {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::ResourceLimit) {
            quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    if context.detail.reason == Basis::ResourceLimit
        || time.timeline.detail.reason == Basis::ResourceLimit
    {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::ResourceLimit) {
            quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    let interval_pages = if profile == PrivacyProfile::Local {
        navigation::build(
            snapshot,
            target,
            &time.timeline,
            evidence.as_ref().map(|e| e.events.as_slice()),
            fallback,
            cancelled,
        )?
    } else {
        // Local cursor work and its budget are not part of a sharing computation.
        navigation::unavailable(time.timeline.track_count.clone(), Basis::UnsupportedMethod)
    };
    if interval_pages.detail.reason == Basis::ResourceLimit {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::ResourceLimit) {
            quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    let repeat_pages = if profile == PrivacyProfile::Local {
        repeats::build(
            snapshot,
            target,
            &a.repeated_behavior,
            evidence.as_ref().map(|e| e.events.as_slice()),
            fallback,
            cancelled,
        )?
    } else {
        repeats::unavailable(
            time.repeated_behavior.combined_operation_count.clone(),
            Basis::UnsupportedMethod,
        )
    };
    if repeat_pages.detail.reason == Basis::ResourceLimit {
        quality.partial = true;
        if !quality.reason_codes.contains(&Basis::ResourceLimit) {
            quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    let (work_projection, work_fallback) = if let Some(e) = &evidence {
        match work::project(work::Input {
            thread,
            turn: turn_id,
            source,
            operations: &e.operations,
            budget: work::Budget::default(),
            cancelled,
        }) {
            Ok(p) => (Some(p), None),
            Err(error)
                if error
                    .downcast_ref::<crate::dto::OperationError>()
                    .is_some_and(|e| e.code == "RESOURCE_LIMIT") =>
            {
                (None, Some(Basis::ResourceLimit))
            }
            Err(error) => return Err(error),
        }
    } else {
        (None, fallback)
    };
    let operation_refs = if work_projection.is_some() {
        vec!["collection:canonical_operations".into()]
    } else {
        vec![]
    };
    let work = m::work(
        &a,
        &turn_refs,
        &operation_refs,
        work_projection.as_ref(),
        fallback,
        work_fallback,
    );
    let mut work_reasons = Vec::new();
    if let Some(p) = &work_projection {
        for count in [
            &p.operation_candidates,
            &p.closed_operations,
            &p.failed_operations,
            &p.file_change_records,
            &p.changed_files,
        ] {
            for gap in &count.gaps {
                work_reasons.push(m::work_gap(*gap));
            }
        }
        if p.coverage.time_gaps > 0 {
            work_reasons.push(Basis::MissingTime);
        }
        if p.coverage.source != work::SourceCoverage::Complete {
            work_reasons.push(Basis::SourcePartial);
        }
    }
    if let Some(reason) = work_fallback {
        work_reasons.push(reason);
    }
    if a.coverage.unclassified_user_records > 0 {
        work_reasons.push(Basis::UnknownMessageOrigin);
    }
    if a.coverage.unassigned_events > 0
        || coverage
            .thread_unassigned_total
            .value
            .is_some_and(|n| n > 0)
    {
        work_reasons.push(Basis::MissingTurn);
    }
    for reason in work_reasons {
        quality.partial = true;
        if !quality.reason_codes.contains(&reason) {
            quality.reason_codes.push(reason);
        }
    }
    let findings = quality
        .reason_codes
        .iter()
        .map(|reason| Finding {
            code: code(*reason).into(),
            kind: FindingKind::Fact,
            metric_refs: vec!["quality".into()],
            evidence_refs: Vec::new(),
        })
        .collect();
    if native.is_some() {
        time.native_wall_clock_ms.evidence_refs = vec!["collection:native_boundary_index".into()];
        time.native_ttft_ms.evidence_refs = vec!["collection:native_boundary_index".into()];
    }
    let mut local = LocalResponse {
        uses: object_uses,
        output_version: OUTPUT_VERSION,
        action: SummaryAction::Summary,
        method_version: METHOD_VERSION.into(),
        profile: LocalProfile::Local,
        privacy: Privacy {
            profile: PrivacyProfile::Local,
            omitted_fields: vec!["source_bodies".into()],
            aliases: "local".into(),
        },
        read_view: ReadView {
            snapshot_id: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
            snapshot_schema: snapshot.manifest.schema_version,
            created_at: snapshot.manifest.snapshot_ref.created_at.clone(),
            adapter_versions: source
                .map_or_else(Vec::new, |source| vec![source.adapter_version.clone()]),
            projection_version: 1,
        },
        scope: local_scope,
        capabilities: m::capabilities(),
        anchors: Anchors {
            start_ms: m::signed(
                a.start.as_ref().map(|start| i128::from(start.timestamp_ms)),
                Basis::ExplicitBoundary,
                &turn_refs,
            ),
            end_ms: m::signed(
                a.end.as_ref().map(|end| i128::from(end.timestamp_ms)),
                Basis::ExplicitBoundary,
                &turn_refs,
            ),
        },
        time,
        context,
        work,
        findings,
        coverage,
        quality,
        freshness,
        evidence: EvidenceIndex {
            repeat_pages,
            interval_pages,
            collections: {
                let mut collections = Vec::new();
                let mut add = |kind, name: &str, count: usize, method: &str| {
                    collections.push(EvidenceCollection {
                        reference: format!("collection:{name}"),
                        kind,
                        snapshot_id: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
                        scope: local_scope_for_collection(thread, turn_id),
                        count: m::observed(count, &[]),
                        method: method.into(),
                    })
                };
                if let Some(e) = &evidence {
                    add(
                        CollectionKind::TurnEvents,
                        "turn_events",
                        e.events.len(),
                        METHOD_VERSION,
                    );
                    add(
                        CollectionKind::CanonicalMeasurements,
                        "canonical_measurements",
                        e.measurements.len(),
                        "reconciled_accounting_v1",
                    );
                    add(
                        CollectionKind::CanonicalOperations,
                        "canonical_operations",
                        e.operations.len(),
                        "canonical_object_use_v1",
                    );
                    add(
                        CollectionKind::SourceControls,
                        "source_controls",
                        e.controls.len(),
                        "physical_domain_controls_v1",
                    );
                }
                if let Some(native) = native {
                    add(
                        CollectionKind::NativeBoundaryIndex,
                        "native_boundary_index",
                        native.candidates,
                        analysis::BOUNDARY_METHOD_VERSION,
                    );
                }
                collections
            },
            available: true,
            limit: 200,
            snapshot_id: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
            refs,
            method: "safe_event_page_v1".into(),
        },
    };
    check(cancelled)?;
    if !fits(&local)? && !local.evidence.repeat_pages.entries.is_empty() {
        // Local proof navigation must not evict independent aggregate facts.
        repeats::omit(&mut local.evidence.repeat_pages, Basis::ResourceLimit);
        local.quality.partial = true;
        if !local.quality.reason_codes.contains(&Basis::ResourceLimit) {
            local.quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    if !fits(&local)? && (!local.uses.objects.is_empty() || local.uses.next_cursor.is_some()) {
        // New detail cannot evict already verified timing/context facts.
        uses::omit_detail(&mut local.uses, Basis::ResourceLimit);
        local.quality.partial = true;
        if !local.quality.reason_codes.contains(&Basis::ResourceLimit) {
            local.quality.reason_codes.push(Basis::ResourceLimit);
        }
    }
    if !fits(&local)? {
        // Never publish prefix distributions. Retain independently observed native
        // scalars; all derived data becomes an explicit budget gap.
        let mut unavailable = analysis::analyze(analysis::AnalyzeInput {
            source: target.source,
            thread: thread_id,
            turn: turn_id,
            measurements: &[],
            events: &[],
            budget: analysis::Budget {
                events: 0,
                measurements: 0,
                lifecycle_records: 0,
            },
        });
        unavailable.context = None;
        unavailable.state = a.state;
        unavailable.native_wall_clock_ms = a.native_wall_clock_ms;
        unavailable.native_ttft_ms = a.native_ttft_ms;
        local.time = m::time(&unavailable, &[], Some(Basis::ResourceLimit), native);
        if native.is_some() {
            local.time.native_wall_clock_ms.evidence_refs =
                vec!["collection:native_boundary_index".into()];
            local.time.native_ttft_ms.evidence_refs =
                vec!["collection:native_boundary_index".into()];
        }
        local.anchors = Anchors {
            start_ms: m::signed(None, Basis::ResourceLimit, &[]),
            end_ms: m::signed(None, Basis::ResourceLimit, &[]),
        };
        local.context = m::context(&unavailable, &[], Some(Basis::ResourceLimit));
        local.work = m::work(
            &unavailable,
            &[],
            &[],
            None,
            Some(Basis::ResourceLimit),
            Some(Basis::ResourceLimit),
        );
        local.evidence.refs.clear();
        local.evidence.repeat_pages =
            repeats::unavailable(m::unavailable(Basis::ResourceLimit), Basis::ResourceLimit);
        local.evidence.interval_pages =
            navigation::unavailable(m::unavailable(Basis::ResourceLimit), Basis::ResourceLimit);
        local.quality.partial = true;
        if !local.quality.reason_codes.contains(&Basis::ResourceLimit) {
            local.quality.reason_codes.push(Basis::ResourceLimit);
        }
        if !fits(&local)? {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Timing summary exceeds output budget",
            ));
        }
    }
    check(cancelled)?;
    let response = if profile == PrivacyProfile::ShareV1 {
        Response::Share(Box::new(share::project(&local)))
    } else {
        Response::Local(Box::new(local))
    };
    if !fits(&response)? {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Timing summary exceeds output budget",
        ));
    }
    check(cancelled)?;
    if let Some(key) = cache_key
        && let Response::Local(local) = &response
    {
        snapshot
            .timing_cache
            .lock()
            .map_err(|_| operation_error("CORE_ERROR", "Timing cache unavailable"))?
            .insert_cancellable(key, *local.clone(), cancelled)?;
    }
    Ok(response)
}
fn local_scope_for_collection(
    thread: &crate::adapters::contract::Thread,
    turn: &str,
) -> LocalScope {
    LocalScope {
        source_instance_id: thread.source_instance_id.clone(),
        thread_id: thread.id.clone(),
        turn_id: turn.into(),
        agent_kind: thread.agent_kind.clone(),
        whole_turn: true,
    }
}
fn code(basis: Basis) -> &'static str {
    match basis {
        Basis::ResourceLimit => "RESOURCE_LIMIT",
        Basis::BoundaryConflict => "TIMING_BOUNDARY_CONFLICT",
        Basis::SourcePartial => "SOURCE_PARTIAL",
        Basis::NumericRange => "NUMERIC_RANGE",
        _ => "TIMING_DETAIL_UNAVAILABLE",
    }
}
fn coverage(
    a: &analysis::Analysis,
    e: Option<&usage_store::timing_evidence::TimingEvidence<'_>>,
    source_status: Option<&str>,
    fallback: Option<Basis>,
) -> Coverage {
    let observed = |value| fallback.map_or_else(|| m::observed(value, &[]), m::unavailable);
    let domains = e.map(|e| e.domains.as_slice());
    Coverage {
        facts: e.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |e| m::observed(e.coverage.facts, &[]),
        ),
        bytes: m::count(
            e.map(|e| u128::from(e.coverage.bytes)),
            fallback.unwrap_or(Basis::SafeEventCount),
            &[],
        ),
        metadata: e.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |e| m::observed(e.coverage.metadata, &[]),
        ),
        event_blocks: e.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |e| m::observed(e.coverage.event_blocks, &[]),
        ),
        scoped_events: observed(a.coverage.scoped_events),
        scoped_measurements: observed(a.coverage.scoped_measurements),
        boundary_candidates: observed(a.coverage.boundary_candidates),
        lifecycle_candidates: a
            .coverage
            .lifecycle_candidates
            .iter()
            .map(|n| observed(*n))
            .collect(),
        linked_lifecycles: a
            .coverage
            .linked_lifecycles
            .iter()
            .map(|n| observed(*n))
            .collect(),
        conflicting_lifecycles: observed(a.coverage.conflicting_lifecycles),
        missing_identity_lifecycles: observed(a.coverage.missing_identity_lifecycles),
        content_candidates: observed(a.coverage.content_candidates),
        domain_count: domains.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |d| m::observed(d.len(), &[]),
        ),
        missing_watermarks: domains.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |d| m::observed(d.iter().filter(|d| d.watermark.is_none()).count(), &[]),
        ),
        generation_mismatches: domains.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |d| m::observed(d.iter().filter(|d| !d.generation_matches).count(), &[]),
        ),
        incomplete_domains: domains.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |d| {
                m::observed(
                    d.iter()
                        .filter(|d| {
                            d.watermark.is_none_or(|w| {
                                w.state != crate::adapters::contract::WatermarkState::Complete
                            }) || !d.generation_matches
                        })
                        .count(),
                    &[],
                )
            },
        ),
        snapshot_unassigned_total: e.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |e| m::observed(e.snapshot_unassigned_total, &[]),
        ),
        thread_unassigned_total: e.map_or_else(
            || m::unavailable(Basis::ResourceLimit),
            |e| m::observed(e.thread_unassigned_total, &[]),
        ),
        source_status: match source_status {
            Some("complete") => "complete",
            Some("partial") => "partial",
            Some("failed") => "failed",
            Some("notFound") => "not_found",
            Some("cancelled") => "cancelled",
            _ => "unknown",
        }
        .into(),
    }
}
fn encode_cursor(cursor: EventCursor) -> Result<Cursor> {
    let bytes = serde_json::to_vec(&cursor)?;
    if bytes.len() > MAX_CURSOR_BYTES / 2 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Timing evidence cursor exceeds budget",
        ));
    }
    Ok(Cursor {
        token: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}
fn decode_cursor(cursor: &Cursor) -> Result<EventCursor> {
    if cursor.token.is_empty()
        || cursor.token.len() > MAX_CURSOR_BYTES
        || !cursor.token.len().is_multiple_of(2)
        || !cursor.token.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return invalid();
    }
    let bytes = cursor
        .token
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    serde_json::from_slice(&bytes)
        .map_err(|_| operation_error("INVALID_ARGUMENT", "Invalid timing evidence cursor"))
}
fn enum_code<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}
fn safe_timestamp(value: Option<i64>) -> (Option<i64>, bool) {
    let numeric_range = value.is_some_and(|value| value.unsigned_abs() > MAX_SAFE_INTEGER);
    (value.filter(|_| !numeric_range), numeric_range)
}
fn evidence_row(event: &Arc<Event>) -> Result<EvidenceRow> {
    let mut row = EvidenceRow {
        reference: format!("event:{}", event.id()),
        record_kind: "unknown".into(),
        timestamp_ms: safe_timestamp(
            event
                .time()
                .timestamp
                .as_deref()
                .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
                .map(|time| time.timestamp_millis()),
        )
        .0,
        phase: None,
        presence: None,
        origin: None,
        duration_ms: None,
        first_token_ms: None,
        gap_codes: event.gaps().iter().map(enum_code).collect(),
    };
    if safe_timestamp(
        event
            .time()
            .timestamp
            .as_deref()
            .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
            .map(|time| time.timestamp_millis()),
    )
    .1
    {
        row.gap_codes.push("numeric_range".into());
    }
    match event.payload() {
        Payload::Lifecycle {
            lifecycle,
            phase,
            duration_ms,
            first_token_ms,
            ..
        } => {
            row.record_kind = enum_code(lifecycle);
            row.phase = Some(enum_code(phase));
            if duration_ms.is_some_and(|n| n > MAX_SAFE_INTEGER)
                || first_token_ms.is_some_and(|n| n > MAX_SAFE_INTEGER)
            {
                row.gap_codes.push("numeric_range".into());
            }
            row.duration_ms = duration_ms.filter(|n| *n <= MAX_SAFE_INTEGER);
            row.first_token_ms = first_token_ms.filter(|n| *n <= MAX_SAFE_INTEGER);
        }
        Payload::Item {
            item_kind, phase, ..
        } => {
            row.record_kind = enum_code(item_kind);
            row.phase = Some(enum_code(phase));
        }
        Payload::Message {
            origin,
            presence,
            record_phase,
            record_kind,
            ..
        } => {
            row.record_kind = enum_code(record_kind);
            row.phase = Some(enum_code(record_phase));
            row.origin = Some(enum_code(origin));
            row.presence = Some(enum_code(presence));
        }
        Payload::Measurement { .. } => row.record_kind = "measurement".into(),
        Payload::Operation { phase, .. } => {
            row.record_kind = "operation".into();
            row.phase = Some(enum_code(phase));
        }
        Payload::ContextWindow { .. } => row.record_kind = "context_window".into(),
        Payload::Activity { .. } => row.record_kind = "activity".into(),
        Payload::Thread { .. } => row.record_kind = "thread".into(),
        Payload::Turn { .. } => row.record_kind = "turn".into(),
        Payload::Ancestry { .. } => row.record_kind = "ancestry".into(),
        Payload::Review { .. } => row.record_kind = "review_observation".into(),
    }
    Ok(row)
}
struct BoundedSize {
    bytes: usize,
    limit: usize,
}
impl Write for BoundedSize {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("summary size"))?;
        if self.bytes > self.limit {
            return Err(std::io::Error::other("summary budget"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn fits(value: &impl serde::Serialize) -> Result<bool> {
    fits_limit(value, MAX_SUMMARY_BYTES)
}
fn fits_limit(value: &impl serde::Serialize, limit: usize) -> Result<bool> {
    let mut size = BoundedSize {
        bytes: 0,
        limit: limit.min(MAX_SUMMARY_BYTES),
    };
    match serde_json::to_writer(&mut size, value) {
        Ok(()) => Ok(true),
        Err(_error) if size.bytes > size.limit => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub fn error_output(error: &anyhow::Error) -> TimingErrorOutput {
    let code = error
        .chain()
        .find_map(|error| {
            error
                .downcast_ref::<crate::dto::OperationError>()
                .map(|error| error.code)
        })
        .unwrap_or("SOURCE_UNREADABLE");
    let (code, message) = match code {
        "INVALID_ARGUMENT" => ("INVALID_ARGUMENT", "Invalid timing request"),
        "EVIDENCE_UNAVAILABLE" | "DATA_REVISION_MISMATCH" | "NO_SNAPSHOT" | "NOT_FOUND" => (
            "EVIDENCE_UNAVAILABLE",
            "Timing read identity is unavailable",
        ),
        "SNAPSHOT_CORRUPT" => ("SNAPSHOT_CORRUPT", "Timing snapshot cannot be read"),
        "RESOURCE_LIMIT" => ("RESOURCE_LIMIT", "Timing query exceeds resource budget"),
        "CANCELLED" => ("CANCELLED", "Timing query cancelled"),
        "UNSUPPORTED_VERSION" => (
            "UNSUPPORTED_VERSION",
            "Timing storage version is unsupported",
        ),
        "TIMING_BOUNDARY_CONFLICT" => ("TIMING_BOUNDARY_CONFLICT", "Timing boundaries disagree"),
        "CORE_ERROR" => ("CORE_ERROR", "Timing query failed"),
        _ => ("SOURCE_UNREADABLE", "Timing source cannot be read"),
    };
    TimingErrorOutput {
        output_version: OUTPUT_VERSION,
        error: TimingError {
            code: code.into(),
            message: message.into(),
        },
    }
}
fn safe_error(error: anyhow::Error) -> anyhow::Error {
    let output = error_output(&error);
    let code = match output.error.code.as_str() {
        "INVALID_ARGUMENT" => "INVALID_ARGUMENT",
        "EVIDENCE_UNAVAILABLE" => "EVIDENCE_UNAVAILABLE",
        "SNAPSHOT_CORRUPT" => "SNAPSHOT_CORRUPT",
        "RESOURCE_LIMIT" => "RESOURCE_LIMIT",
        "CANCELLED" => "CANCELLED",
        "UNSUPPORTED_VERSION" => "UNSUPPORTED_VERSION",
        "TIMING_BOUNDARY_CONFLICT" => "TIMING_BOUNDARY_CONFLICT",
        "CORE_ERROR" => "CORE_ERROR",
        _ => "SOURCE_UNREADABLE",
    };
    operation_error(code, output.error.message)
}

#[cfg(test)]
#[path = "query/tests.rs"]
mod tests;
