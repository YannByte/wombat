use super::*;

#[test]
fn verified_native_execution_counts_without_promoting_display_only_read_labels() {
    let mut native = use_operation("native", "command", "failed");
    native.name = "exec_command".into();
    native.path = None;
    native.work=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::WORK_OBSERVATION_VERSION,"stage":"terminal","data":{"kind":"command","cwd":"/synthetic","source":"unified_exec_startup","parsed_commands":[{"kind":"read","path":"SKILL.md"}]},"gaps":[]})).unwrap());
    native.matching=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::OPERATION_MATCH_VERSION,"receiverOwner":null,"requestFingerprint":"0".repeat(64),"readTargets":[{"path":"/synthetic/SKILL.md","platform":"posix"}],"expectedNonzero":false,"gaps":["missing_receiver"]})).unwrap());
    let mut weak = native.clone();
    weak.id = "display-only".into();
    weak.call_id = Some("display-only".into());
    weak.matching = None;
    let v = use_view_paths(vec![native.clone(), weak], true);
    let result = execute(request(), String::new(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(1));
    assert_eq!(result.items[0].counts.file_reads, 1);
    assert_eq!(result.items[0].counts.failed, 1);
    let explanation = result.explanation.unwrap();
    assert_eq!(explanation.baseline.counted, 1);
    assert!(
        explanation
            .records
            .iter()
            .any(|record| record.operation_id == "display-only"
                && record.reason == UseReason::DispatchUnknown)
    );
    native.status = "declined".into();
    let declined = execute(
        request(),
        String::new(),
        &use_view_paths(vec![native], true),
    )
    .unwrap();
    assert_eq!(declined.items[0].usage_count, Some(0));
}

#[test]
fn native_windows_verified_targets_match_inventory_drive_spelling_on_every_host() {
    let mut native = use_operation("native-windows", "command", "completed");
    native.name = "exec_command".into();
    native.work=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::WORK_OBSERVATION_VERSION,"stage":"terminal","data":{"kind":"command","cwd":"file:///C:/Synthetic","source":"unified_exec_startup","parsed_commands":[{"kind":"read","path":"SKILL.md"}]},"gaps":[]})).unwrap());
    native.matching=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::OPERATION_MATCH_VERSION,"receiverOwner":null,"requestFingerprint":"0".repeat(64),"readTargets":[{"path":"c:/Synthetic/SKILL.md","platform":"windows"}],"expectedNonzero":false,"gaps":["missing_receiver"]})).unwrap());
    let mut v = use_view_paths(vec![native], true);
    v.items[0].path = r"C:\Synthetic\SKILL.md".into();
    let result = execute(request(), String::new(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(1));
    assert_eq!(result.items[0].path, r"C:\Synthetic\SKILL.md");
}

fn request() -> Request {
    Request {
        action: Action::Explain,
        kind: Some(Kind::Skill),
        item_id: Some("skill".into()),
        ..uses_request()
    }
}

#[test]
fn unrelated_known_object_updates_do_not_invalidate_a_selected_use_comparison() {
    let mut a = use_operation("a", "skillRead", "completed");
    a.path = Some("/synthetic/SKILL.md".into());
    let mut b = use_operation("b", "skillRead", "completed");
    b.path = Some("/synthetic/other/SKILL.md".into());
    let make = |ops| {
        let mut v = use_view_paths(ops, true);
        let mut other = v.items[0].clone();
        other.id = "other".into();
        other.path = "/synthetic/other/SKILL.md".into();
        v.items.push(other);
        v
    };
    let first = execute(request(), String::new(), &make(vec![a.clone()]))
        .unwrap()
        .explanation
        .unwrap();
    let next = execute(
        Request {
            baseline: Some(Box::new(first.baseline.clone())),
            baseline_records: Some(first.records.clone()),
            ..request()
        },
        String::new(),
        &make(vec![a.clone(), b.clone()]),
    )
    .unwrap()
    .explanation
    .unwrap();
    assert_eq!(
        next.comparison.unwrap().status,
        UseComparisonStatus::Unchanged
    );
    b.id = a.id.clone();
    b.call_id = a.call_id.clone();
    let conflict = execute(
        Request {
            baseline: Some(Box::new(first.baseline)),
            baseline_records: Some(first.records),
            ..request()
        },
        String::new(),
        &make(vec![a, b]),
    )
    .unwrap()
    .explanation
    .unwrap();
    assert_eq!(
        conflict.comparison.unwrap().status,
        UseComparisonStatus::InputChanged
    );
}

#[test]
fn byte_read_completion_is_separate_from_operation_attribution_and_source_messages_are_not_exposed()
{
    let mut v = use_view(vec![]);
    let snapshot = Arc::get_mut(v.snapshot.as_mut().unwrap()).unwrap();
    let issue = crate::adapters::contract::Issue {
        code: "workObservationPartial".into(),
        message: "SYNTHETIC_PRIVATE_DIAGNOSTIC".into(),
        source_instance_id: Some("source".into()),
        evidence: Some(crate::adapters::contract::EvidenceRef {
            file: "/synthetic/source.jsonl".into(),
            line: 7,
        }),
    };
    snapshot
        .manifest
        .sources
        .push(crate::adapters::contract::SourceReport {
            source: crate::adapters::contract::SourceInstance {
                id: "source".into(),
                agent_kind: "codex".into(),
                root: "/synthetic".into(),
            },
            adapter_version: "synthetic".into(),
            source_versions: vec![],
            capabilities: Default::default(),
            status: "partial".into(),
            files_read: 1,
            bytes_read: 100,
            issues: vec![issue.clone()],
        });
    snapshot.manifest.issues.push(issue);
    snapshot.manifest.watermarks.push(serde_json::from_value(serde_json::json!({"formatVersion":2,"sourceInstanceId":"source","fileId":"synthetic","committedOffset":100,"observedBytes":100,"observedAt":"2026-11-03T00:00:00Z","state":"partial","issueCodes":["invalidSourceField"]})).unwrap());
    let result = execute(request(), String::new(), &v).unwrap();
    assert_eq!(result.coverage.history_status, "partial");
    assert_eq!(result.coverage.history_read_complete, Some(true));
    assert_eq!(
        result.explanation.as_ref().unwrap().source_issues[0]
            .evidence
            .as_ref()
            .unwrap()
            .line,
        7
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("SYNTHETIC_PRIVATE_DIAGNOSTIC")
    );
    Arc::get_mut(v.snapshot.as_mut().unwrap())
        .unwrap()
        .manifest
        .watermarks[0]
        .observed_bytes = Some(101);
    assert_eq!(
        execute(request(), String::new(), &v)
            .unwrap()
            .coverage
            .history_read_complete,
        Some(false)
    );
}

#[test]
fn explanation_is_the_actual_counting_path_and_retains_only_the_requested_page() {
    let read = use_operation("read", "skillRead", "failed");
    let mut anonymous = use_operation("anonymous", "skillRead", "completed");
    anonymous.call_id = None;
    let mut candidate = use_operation("candidate", "skillRead", "completed");
    candidate.name = "read_skill_file".into();
    let v = use_view(vec![read.clone(), read, anonymous, candidate]);
    let normal = execute(uses_request(), String::new(), &v).unwrap();
    assert!(normal.explanation.is_none());
    let mut cached_view = use_view(vec![use_operation("cached", "skillRead", "completed")]);
    cached_view.history_status = "cached".into();
    assert_eq!(
        execute(uses_request(), String::new(), &cached_view)
            .unwrap()
            .items[0]
            .usage_count,
        Some(1)
    );
    let r = Request {
        limit: Some(1),
        ..request()
    };
    let first = execute(r.clone(), String::new(), &v).unwrap();
    assert_eq!(first.items[0].usage_count, Some(1));
    assert_eq!(
        first.items[0].counts.file_reads,
        normal.items[0].counts.file_reads
    );
    let trace = first.explanation.unwrap();
    assert_eq!(trace.baseline.counted, 1);
    assert_eq!(trace.baseline.total_records, 4);
    assert_eq!(trace.records.len(), 1);
    assert_eq!(trace.page.next_offset, Some(1));
    for reason in [
        UseReason::Counted,
        UseReason::Duplicate,
        UseReason::IdentityMissing,
        UseReason::DispatchUnknown,
    ] {
        assert!(
            trace
                .baseline
                .reasons
                .iter()
                .any(|row| row.reason == reason && row.records == 1)
        );
    }
    let next = execute(
        Request {
            offset: Some(1),
            ..r
        },
        String::new(),
        &v,
    )
    .unwrap()
    .explanation
    .unwrap();
    assert_eq!(next.records[0].reason, UseReason::Duplicate);
    assert_eq!(next.records[0].count_delta, Some(0));
    assert_eq!(next.baseline.counted, 1);
    assert_eq!(trace.baseline.input_revision, next.baseline.input_revision);
    assert_eq!(
        trace.baseline.result_revision,
        next.baseline.result_revision
    );
}

#[test]
fn replay_is_deterministic_and_differences_require_identical_inputs_and_scope() {
    let mut anonymous = use_operation("anonymous", "skillRead", "completed");
    anonymous.call_id = None;
    let mut v = use_view(vec![
        use_operation("read", "skillRead", "completed"),
        anonymous,
    ]);
    let first = execute(request(), String::new(), &v)
        .unwrap()
        .explanation
        .unwrap();
    let mut r = Request {
        baseline: Some(Box::new(first.baseline.clone())),
        baseline_records: Some(first.records.clone()),
        ..request()
    };
    v.checked = "2026-11-03T00:00:00Z".into();
    let next = execute(r.clone(), String::new(), &v)
        .unwrap()
        .explanation
        .unwrap();
    let comparison = next.comparison.unwrap();
    assert_eq!(comparison.status, UseComparisonStatus::Unchanged);
    assert_eq!(comparison.delta, Some(0));
    assert!(comparison.changes.is_empty());
    assert!(comparison.changes_complete);
    let old = r.baseline.as_mut().unwrap();
    old.counted = 2;
    old.method_version = 2;
    old.result_revision = "0".repeat(64);
    let record = r
        .baseline_records
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record.operation_id == "anonymous")
        .unwrap();
    record.reason = UseReason::Counted;
    record.count_delta = Some(1);
    let comparison = execute(r.clone(), String::new(), &v)
        .unwrap()
        .explanation
        .unwrap()
        .comparison
        .unwrap();
    assert_eq!(comparison.status, UseComparisonStatus::Changed);
    assert_eq!(comparison.delta, Some(-1));
    assert!(
        comparison
            .changes
            .iter()
            .any(|change| change.operation_id == "anonymous"
                && change.after == Some(UseReason::IdentityMissing))
    );
    r.baseline.as_mut().unwrap().input_revision = "0".repeat(64);
    let comparison = execute(r.clone(), String::new(), &v)
        .unwrap()
        .explanation
        .unwrap()
        .comparison
        .unwrap();
    assert_eq!(comparison.status, UseComparisonStatus::InputChanged);
    assert!(comparison.delta.is_none());
    r.baseline.as_mut().unwrap().scope.project = Some("/different".into());
    let comparison = execute(r.clone(), String::new(), &v)
        .unwrap()
        .explanation
        .unwrap()
        .comparison
        .unwrap();
    assert_eq!(comparison.status, UseComparisonStatus::ScopeChanged);
    assert!(comparison.delta.is_none());
    r.baseline.as_mut().unwrap().scope.project = None;
    r.baseline.as_mut().unwrap().configuration_revision = "0".repeat(64);
    assert_eq!(
        execute(r, String::new(), &v)
            .unwrap()
            .explanation
            .unwrap()
            .comparison
            .unwrap()
            .status,
        UseComparisonStatus::ConfigurationChanged
    );
}

#[test]
fn unassigned_records_have_original_positions_without_becoming_uses_of_every_item() {
    let mut op = use_operation("unbound", "skillRead", "completed");
    op.evidence.push(crate::adapters::contract::EvidenceRef {
        file: "/synthetic/source.jsonl".into(),
        line: 7,
    });
    let v = use_view_paths(vec![op], true);
    let request = Request {
        item_id: None,
        unassigned: true,
        ..request()
    };
    let result = execute(request, String::new(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(0));
    let explanation = result.explanation.unwrap();
    assert_eq!(explanation.baseline.counted, 0);
    assert_eq!(explanation.records.len(), 1);
    assert_eq!(explanation.records[0].reason, UseReason::TargetMissing);
    assert_eq!(explanation.records[0].item_id, None);
    assert_eq!(explanation.records[0].evidence.as_ref().unwrap().line, 7);
}

#[test]
fn date_exclusions_conflicts_and_unknown_baselines_are_explicit() {
    let mut op = use_operation("outside", "skillRead", "completed");
    op.timestamp = Some("2026-09-01T00:00:00Z".into());
    let v = use_view(vec![op]);
    let r = Request {
        scope: Scope {
            since: Some("2026-10-01".into()),
            until: Some("2026-10-06".into()),
            ..Default::default()
        },
        ..request()
    };
    let result = execute(r.clone(), String::new(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(0));
    assert_eq!(
        result.explanation.as_ref().unwrap().records[0].reason,
        UseReason::OutsideWindow
    );
    let mut baseline = result.explanation.unwrap().baseline;
    baseline.format_version = 99;
    assert!(
        execute(
            Request {
                baseline: Some(Box::new(baseline)),
                ..r
            },
            String::new(),
            &v
        )
        .unwrap_err()
        .to_string()
        .contains("不支持此计数解释基线")
    );
}
