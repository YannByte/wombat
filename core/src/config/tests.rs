use super::*;
mod trace;
#[test]
fn review_catalog_metadata_is_independent_from_body_and_never_confirms_visibility() {
    let mut v = use_view(vec![]);
    v.projects = vec!["/project-a".into(), "/project-b".into()];
    let path = v.items[0].path.clone();
    let capture = review::Capture {
        native_version: Some("0.166.0-alpha.5.1".into()),
        checked_at: Some(Utc::now().to_rfc3339()),
        error_code: None,
        contexts: v
            .projects
            .iter()
            .enumerate()
            .map(|(n, p)| review::CapturedContext {
                cwd: p.clone(),
                complete: true,
                skills: vec![review::CapturedSkill {
                    name: "synthetic:skill".into(),
                    description: "Exact local evidence".into(),
                    path: path.clone(),
                    enabled: n == 0,
                    scope: "user".into(),
                }],
            })
            .collect(),
    };
    review::bind(&mut v.items, Some(&capture), &v.projects);
    let first = v.items[0].context_footprint.clone().unwrap();
    assert_eq!(
        first.estimate.as_ref().unwrap().payload,
        "skillDiscoveryMetadata"
    );
    assert_eq!(
        first.estimate.as_ref().unwrap().applicability,
        "referenceEncodingOnly"
    );
    assert_eq!(first.visibility, "unconfirmed");
    assert_eq!(first.contexts.len(), 2);
    v.items[0].content_tokens = Some(999999);
    v.items[0].bytes = Some(9999999);
    review::bind(&mut v.items, Some(&capture), &v.projects);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&v.items[0].context_footprint).unwrap()
    );
    assert_eq!(review::summary(&v.items).confirmed_visible_tokens, None);
    let before = observations::derive(&v).unwrap();
    v.items[0].context_footprint.as_mut().unwrap().contexts[0].enabled = false;
    let after = observations::derive(&v).unwrap();
    assert_eq!(before.config_revision, after.config_revision);
    assert_ne!(before.host_revision, after.host_revision);
    assert_eq!(review::summary(&v.items).disabled_items, 1);
    let mut conflict = capture.clone();
    conflict.contexts[1].skills[0].description = "conflict".into();
    review::bind(&mut v.items, Some(&conflict), &v.projects);
    assert!(
        v.items[0]
            .context_footprint
            .as_ref()
            .unwrap()
            .estimate
            .is_none()
    );
    review::bind(&mut v.items, None, &v.projects);
    assert_eq!(review::summary(&v.items).unconfirmed_items, 1);
    for kind in [Kind::Mcp, Kind::Hook] {
        v.items[0].kind = kind;
        review::bind(&mut v.items, Some(&capture), &v.projects);
        assert!(review::tokens(&v.items[0]).is_none());
    }
}

#[test]
fn review_unknowns_follow_known_zero_and_paging_preserves_totals() {
    let mut v = use_view(vec![use_operation("read", "skillRead", "failed")]);
    let mut other = v.items[0].clone();
    other.id = "unobserved".into();
    other.path = "/different/SKILL.md".into();
    v.items.push(other);
    review::bind(&mut v.items, None, &[]);
    let mut r = uses_request();
    r.review = true;
    r.limit = Some(1);
    let full = execute(
        Request {
            limit: Some(50),
            ..r.clone()
        },
        "config:review".into(),
        &v,
    )
    .unwrap();
    let first = execute(r.clone(), "config:review".into(), &v).unwrap();
    r.offset = Some(1);
    let next = execute(r, "config:review".into(), &v).unwrap();
    assert_eq!(full.page.total, 2);
    assert_eq!(full.items[0].usage_count, Some(0));
    assert_eq!(full.items[1].usage_count, Some(1));
    assert_eq!(first.items[0].id, full.items[0].id);
    assert_eq!(next.items[0].id, full.items[1].id);
    assert_eq!(first.extension_review.unwrap().unconfirmed_items, 2);
    assert_eq!(next.extension_review.unwrap().unmeasured_items, 2);
    let mut a = full.items[0].clone();
    let mut b = a.clone();
    a.usage_count = None;
    b.usage_count = Some(0);
    assert_eq!(review::compare(&a, &b), std::cmp::Ordering::Greater);
    a.usage_count = Some(0);
    a.use_basis.as_mut().unwrap().status = UseBasisStatus::Partial;
    b.usage_count = Some(1);
    b.use_basis.as_mut().unwrap().status = UseBasisStatus::Observed;
    assert_eq!(review::compare(&a, &b), std::cmp::Ordering::Greater);
}
fn view() -> View {
    View {
        observation_versions: Default::default(),
        config_collection: Default::default(),
        hook_registry: HookRegistry::default(),
        snapshot: None,
        items: vec![],
        issues: vec![],
        projects: vec![],
        roots: vec![],
        project_roots: vec![],
        revision: "one".into(),
        checked: "2026-10-01T00:00:00Z".into(),
        history_status: "unavailable".into(),
        analysis: Default::default(),
    }
}
#[test]
fn committed_inventory_preserves_partial_coverage() {
    let root = tempfile::tempdir().unwrap();
    {
        let (issue, expected) = ("syntheticCoverageGap", "partial");
        let mut collected = crate::adapters::contract::Collected::default();
        collected.issues.push(crate::adapters::contract::Issue {
            code: issue.into(),
            message: "synthetic".into(),
            source_instance_id: None,
            evidence: None,
        });
        let snapshot = crate::usage_store::memory(
            collected,
            "live:test:one".into(),
            crate::pricing_sync::current_at(root.path()).unwrap(),
            None,
        )
        .unwrap();
        let mut v = view();
        v.snapshot = Some(Arc::new(snapshot));
        v.history_status = "fixed".into();
        let result = execute(Request::default(), "config:test".into(), &v).unwrap();
        assert_eq!(result.coverage.history_status, expected);
    }
}
#[test]
fn configuration_reads_need_no_retained_inventory() {
    let first = execute(Request::default(), String::new(), &view()).unwrap();
    let second = execute(Request::default(), String::new(), &view()).unwrap();
    assert_eq!(first.items.len(), second.items.len());
    assert!(
        serde_json::to_value(first)
            .unwrap()
            .get("readView")
            .is_none()
    );
}
#[test]
fn local_dates_use_exclusive_end_and_preserve_unknowns() {
    let (scope, tz) = normalize(&Scope {
        since: Some("2026-09-29".into()),
        until: Some("2026-09-30".into()),
        timezone: Some("Asia/Shanghai".into()),
        ..Default::default()
    })
    .unwrap();
    assert!(in_time(Some("2026-09-28T16:00:00Z"), &scope, tz));
    assert!(!in_time(Some("2026-09-29T16:00:00Z"), &scope, tz));
    assert!(!in_time(None, &scope, tz));
    let result = execute(Request::default(), "view".into(), &view()).unwrap();
    assert!(result.summary.usage.is_none());
    assert!(!result.coverage.absence_observable);
    assert_eq!(result.coverage.status, "partial");
}

fn use_operation(id: &str, kind: &str, status: &str) -> crate::adapters::contract::Operation {
    serde_json::from_value(serde_json::json!({
        "id": id, "threadId": "thread", "turnId": "turn", "callId": id,
        "kind": kind, "name": "read_file", "sequence": 1,
        "timestamp": "2026-10-04T00:00:00Z", "timePrecision": "second",
        "status": status, "outcomeConflict":false, "evidence": []
    }))
    .unwrap()
}

fn use_view(operations: Vec<crate::adapters::contract::Operation>) -> View {
    use_view_paths(operations, false)
}
fn use_view_paths(
    mut operations: Vec<crate::adapters::contract::Operation>,
    preserve: bool,
) -> View {
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join("synthetic-skill/SKILL.md")
        .to_string_lossy()
        .into_owned();
    let path = if preserve {
        "/synthetic/SKILL.md".into()
    } else {
        path
    };
    if !preserve {
        for op in &mut operations {
            op.path = Some(path.clone());
        }
    }
    let mut collected = crate::adapters::contract::Collected::default();
    collected.threads.push(crate::adapters::contract::Thread {
        id: "thread".into(),
        agent_kind: "codex".into(),
        source_instance_id: "source".into(),
        upstream_id: "native-thread".into(),
        title: None,
        project: None,
        started_at: None,
        last_activity_at: None,
    });
    collected.operations = operations.into_iter().map(Arc::new).collect();
    let snapshot = crate::usage_store::memory(
        collected,
        "live:uses:one".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let mut v = view();
    v.snapshot = Some(Arc::new(snapshot));
    v.history_status = "fixed".into();
    v.items.push(serde_json::from_value(serde_json::json!({
        "id":"skill", "name":"synthetic-skill", "kind":"skill",
        "sourceInstanceId":"source", "path":path, "authorizedProjects":[],
        "sourceContexts":[{"inventoryId":"skill-source", "global":true,
            "sourceInstanceId":"source", "contentHash":"synthetic", "configuredState":"enabled",
            "counts":{"fileReads":9,"toolCalls":0,"resourceReads":0,"succeeded":9,"failed":0,"outcomeUnknown":0},
            "observation":"used"}],
        "configuredState":"enabled", "contentHash":"synthetic", "observedAt":"now",
        "current":true, "stale":false, "estimateStatus":"unknown",
        "measurementStatus":"unknown", "bodyEstimateStatus":"unknown",
        "usageCount":9, "observation":"used",
        "counts":{"fileReads":9,"toolCalls":0,"resourceReads":0,"succeeded":9,"failed":0,"outcomeUnknown":0},
        "relatedTurns":9, "relatedTasks":9
    })).unwrap());
    v
}

fn uses_request() -> Request {
    Request {
        scope: Scope {
            all_time: Some(true),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn inventory_and_recorded_paths_use_the_same_grammar_without_changing_display_identity() {
    for (inventory, recorded, foreign) in [
        (
            r"C:\synthetic/skill/SKILL.md",
            r"C:\synthetic\skill\SKILL.md",
            r"D:\synthetic\skill\SKILL.md",
        ),
        (
            r"C:\synthetic\skill\SKILL.md",
            "C:/synthetic/skill/SKILL.md",
            "/synthetic/skill/SKILL.md",
        ),
        (
            r"\\server\share/skill/SKILL.md",
            r"\\server\share\skill\SKILL.md",
            r"\\other\share\skill\SKILL.md",
        ),
        (
            "/synthetic/dir/../skill/SKILL.md",
            "/synthetic/skill/SKILL.md",
            r"C:\synthetic\skill\SKILL.md",
        ),
    ] {
        let mut read = use_operation("matching", "skillRead", "completed");
        read.path = Some(recorded.into());
        let mut other = use_operation("foreign", "skillRead", "completed");
        other.path = Some(foreign.into());
        let mut v = use_view_paths(vec![read, other], true);
        v.items[0].path = inventory.into();
        let result = execute(uses_request(), "config:portable".into(), &v).unwrap();
        assert_eq!(result.items[0].usage_count, Some(1), "{inventory}");
        assert_eq!(result.items[0].observation, Observation::Used);
        assert_eq!(result.items[0].path, inventory);
        assert_eq!(result.items[0].id, "skill");
    }
    let mut read = use_operation("ambiguous", "skillRead", "completed");
    read.path = Some(r"C:\synthetic\skill\SKILL.md".into());
    let mut v = use_view_paths(vec![read], true);
    v.items[0].path = r"C:\synthetic/skill/SKILL.md".into();
    let mut duplicate = v.items[0].clone();
    duplicate.id = "duplicate".into();
    duplicate.path = r"C:\synthetic\skill\SKILL.md".into();
    v.items.push(duplicate);
    let result = execute(uses_request(), "config:portable".into(), &v).unwrap();
    for item in result.items {
        assert_eq!(item.usage_count, Some(0));
        assert!(
            result
                .coverage
                .issues
                .iter()
                .any(|issue| issue.code == "usageCountV4TargetUnknown"
                    && issue.path.as_ref() == Some(&item.path))
        );
    }
}

#[test]
fn declarations_and_catalogs_do_not_count_or_keep_cached_used_observations() {
    let v = use_view(vec![
        use_operation("declaration", "skillUse", "completed"),
        use_operation("catalog", "skillAvailable", "completed"),
    ]);
    let result = execute(uses_request(), "config:uses".into(), &v).unwrap();
    let item = &result.items[0];
    assert_eq!(item.usage_count, Some(0));
    assert_eq!(item.observation, Observation::Unknown);
    assert_eq!(item.counts.file_reads, 0);
    assert_eq!((item.related_turns, item.related_tasks), (0, 0));
    assert_eq!(item.source_contexts[0].observation, Observation::Unknown);
    assert_eq!(item.source_contexts[0].counts.file_reads, 0);
}

#[test]
fn query_counts_three_skill_operations_in_one_turn_including_failure() {
    let first = use_operation("first", "skillRead", "completed");
    let v = use_view(vec![
        first.clone(),
        first,
        use_operation("second", "skillRead", "failed"),
        use_operation("third", "skillRead", "running"),
    ]);
    let mut request = uses_request();
    request.action = Action::Evidence;
    request.item_id = Some("skill".into());
    request.limit = Some(1);
    let result = execute(request, "config:uses".into(), &v).unwrap();
    let item = &result.items[0];
    assert_eq!(item.usage_count, Some(3));
    assert_eq!(item.observation, Observation::Used);
    assert_eq!((item.related_turns, item.related_tasks), (1, 1));
    assert_eq!(item.counts.file_reads, 3);
    assert_eq!(
        (
            item.counts.succeeded,
            item.counts.failed,
            item.counts.outcome_unknown
        ),
        (1, 1, 1)
    );
    assert_eq!(item.source_contexts[0].observation, Observation::Used);
    assert_eq!(item.source_contexts[0].counts.failed, 1);
    assert_eq!(result.page.total, 3);
    assert_eq!(result.evidence.len(), 1);
    assert!(item.usage.is_none());
}

#[test]
fn query_projection_keeps_full_scope_activity_when_list_pages_change() {
    let mut v = use_view(vec![use_operation("read", "skillRead", "completed")]);
    let mut unseen = v.items[0].clone();
    unseen.id = "unseen".into();
    unseen.path = "/synthetic/unseen/SKILL.md".into();
    unseen.name = "unseen".into();
    v.items.push(unseen);
    let mut request = uses_request();
    request.sort = Sort::Name;
    request.limit = Some(1);
    let first = execute(request.clone(), "config:first".into(), &v).unwrap();
    request.offset = Some(1);
    let second = execute(request, "config:second".into(), &v).unwrap();
    assert_eq!(first.page.total, 2);
    assert_eq!(second.page.total, 2);
    assert_eq!(first.items.len(), 1);
    assert_eq!(second.items.len(), 1);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_eq!(first.summary.observed_items, 1);
    assert_eq!(second.summary.observed_items, 1);
    for result in [first, second] {
        let activity = result.extension_activity.unwrap();
        assert_eq!(activity.observed_use, 1);
        assert_eq!(activity.no_observed_use, 1);
        assert_eq!(activity.items.len(), 1);
    }
    assert_eq!(v.items[0].source_contexts[0].counts.file_reads, 9);
}

#[test]
fn wrapper_literal_read_keeps_a_candidate_without_claiming_actual_dispatch() {
    let mut wrapper = use_operation("wrapper-path-hash", "skillRead", "completed");
    wrapper.name = "read_skill_file".into();
    wrapper.call_id = None;
    let v = use_view(vec![wrapper]);
    let result = execute(uses_request(), "config:uses".into(), &v).unwrap();
    let item = &result.items[0];
    assert_eq!(item.observation, Observation::Unknown);
    assert_eq!(item.usage_count, Some(0));
    assert_eq!((item.related_turns, item.related_tasks), (0, 0));
    assert_eq!(item.counts.file_reads, 0);
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4DispatchUnknown"
                && issue.path.as_ref() == Some(&item.path))
    );
    let mut request = uses_request();
    request.action = Action::Evidence;
    request.item_id = Some("skill".into());
    let evidence = execute(request, "config:uses".into(), &v).unwrap();
    assert_eq!(evidence.evidence.len(), 1);
    assert_eq!(evidence.evidence[0].event_type, "skill_read_candidate");
    assert_eq!(evidence.evidence[0].association, "operationDispatchUnknown");
    assert!(evidence.evidence[0].usage.is_none());
}

#[test]
fn explicit_read_without_native_identity_is_used_with_a_partial_zero_count() {
    let mut read = use_operation("anonymous-read", "skillRead", "failed");
    read.call_id = None;
    read.turn_id = None;
    let v = use_view(vec![read]);
    let result = execute(uses_request(), "config:uses".into(), &v).unwrap();
    assert_eq!(result.items[0].observation, Observation::Used);
    assert_eq!(
        result.items[0].source_contexts[0].observation,
        Observation::Used
    );
    assert_eq!(result.items[0].usage_count, Some(0));
    assert_eq!(result.items[0].related_turns, 0);
    assert_eq!(result.items[0].related_tasks, 1);
    assert_eq!(result.items[0].counts.file_reads, 1);
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4IdentityUnknown")
    );
}

#[test]
fn mcp_calls_and_resource_reads_share_operation_counts_but_discovery_does_not() {
    let mut ops = vec![
        use_operation("tool", "mcpTool", "failed"),
        use_operation("resource", "mcpResource", "unknown"),
        use_operation("discovery", "mcpDiscovery", "completed"),
    ];
    for op in &mut ops {
        op.server = Some("server".into());
        op.tool = Some("tool".into());
    }
    let mut v = use_view(ops.clone());
    v.items[0].kind = Kind::Mcp;
    v.items[0].native_key = Some("server".into());
    let result = execute(uses_request(), "config:uses".into(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(2));
    assert_eq!(result.items[0].observation, Observation::Used);
    assert_eq!(
        (
            result.items[0].counts.tool_calls,
            result.items[0].counts.resource_reads
        ),
        (1, 1)
    );
    assert_eq!(result.items[0].related_turns, 1);

    // Adapter conflicts deliberately erase unreliable server attribution.
    ops.push(use_operation("conflict", "mcpConflict", "failed"));
    let mut conflict = use_view(ops);
    conflict.items[0].kind = Kind::Mcp;
    conflict.items[0].native_key = Some("server".into());
    let result = execute(uses_request(), "config:uses".into(), &conflict).unwrap();
    assert_eq!(result.items[0].usage_count, Some(2));
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4TargetUnknown")
    );
}

#[test]
fn ambiguous_inventory_targets_keep_candidate_gaps_instead_of_picking_an_item() {
    let mut v = use_view(vec![use_operation("read", "skillRead", "completed")]);
    let mut second_version = v.items[0].clone();
    second_version.id = "second-version".into();
    second_version.content_hash = "different-current-content".into();
    v.items.push(second_version);
    let result = execute(uses_request(), "config:uses".into(), &v).unwrap();
    assert_eq!(result.items.len(), 2);
    for item in &result.items {
        assert_eq!(item.usage_count, Some(0));
        assert_eq!(item.observation, Observation::Unknown);
        assert_eq!(item.counts.file_reads, 0);
        assert!(
            result
                .coverage
                .issues
                .iter()
                .any(|issue| issue.code == "usageCountV4TargetUnknown"
                    && issue.path.as_ref() == Some(&item.path))
        );
    }
}

#[test]
fn missing_time_prevents_a_precise_date_window_count_without_creating_a_turn() {
    let mut missing = use_operation("missing-time", "skillRead", "failed");
    missing.timestamp = None;
    missing.turn_id = None;
    let v = use_view(vec![
        use_operation("known-time", "skillRead", "completed"),
        missing,
    ]);
    let request = Request {
        scope: Scope {
            since: Some("2026-10-04".into()),
            until: Some("2026-10-05".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = execute(request, "config:uses".into(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(1));
    assert_eq!(result.items[0].related_turns, 1);
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4TimeUnknown")
    );
    let all = execute(uses_request(), "config:uses".into(), &v).unwrap();
    assert_eq!(all.items[0].usage_count, Some(2));
    assert_eq!(all.items[0].counts.file_reads, 2);
    assert_eq!(all.items[0].related_turns, 1);
    assert!(
        all.coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4TurnUnknown")
    );
}

#[test]
fn thread_scope_retains_read_candidates_and_their_queryable_evidence() {
    let mut wrapper = use_operation("wrapper-path-hash", "skillRead", "completed");
    wrapper.name = "read_skill_file".into();
    wrapper.call_id = None;
    let v = use_view(vec![wrapper]);
    let mut request = uses_request();
    request.scope.thread_id = Some("thread".into());
    let list = execute(request.clone(), "config:uses".into(), &v).unwrap();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].observation, Observation::Unknown);
    assert_eq!(list.items[0].usage_count, Some(0));
    assert_eq!(list.items[0].counts.file_reads, 0);
    assert_eq!(list.summary.observed_items, 0);
    assert!(
        list.coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4DispatchUnknown")
    );
    request.action = Action::Evidence;
    request.item_id = Some("skill".into());
    let evidence = execute(request, "config:uses".into(), &v).unwrap();
    assert_eq!(evidence.items.len(), 1);
    assert_eq!(evidence.page.total, 1);
    assert_eq!(evidence.evidence[0].event_type, "skill_read_candidate");
    assert_eq!(evidence.evidence[0].association, "operationDispatchUnknown");
    assert!(evidence.evidence[0].usage.is_none());
}

#[test]
fn thread_scope_retains_loaded_instructions_without_counting_a_file_read() {
    let mut load = use_operation("instruction", "instructionLoad", "completed");
    load.call_id = None;
    load.item_id = Some("native-message-document".into());
    let mut view = use_view(vec![load]);
    view.items[0].kind = Kind::Rule;
    let mut request = uses_request();
    request.scope.thread_id = Some("thread".into());
    let result = execute(request.clone(), "config:uses".into(), &view).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].observation, Observation::LoadedOnly);
    assert_eq!(result.items[0].counts.file_reads, 0);
    assert_eq!(result.items[0].source_contexts[0].counts.file_reads, 0);
    assert_eq!(result.items[0].usage_count, None);
    request.scope.thread_id = Some("other-thread".into());
    assert!(
        execute(request, "config:uses".into(), &view)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn thread_scope_retains_missing_time_coverage_without_inventing_evidence_dates() {
    let mut read = use_operation("known-call-without-time", "skillRead", "failed");
    read.timestamp = None;
    let v = use_view(vec![read]);
    let request = Request {
        action: Action::Evidence,
        item_id: Some("skill".into()),
        scope: Scope {
            thread_id: Some("thread".into()),
            since: Some("2026-10-04".into()),
            until: Some("2026-10-05".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = execute(request, "config:uses".into(), &v).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].usage_count, Some(0));
    assert_eq!(result.items[0].counts.file_reads, 0);
    assert!(result.evidence.is_empty());
    assert_eq!(result.page.total, 0);
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "usageCountV4TimeUnknown")
    );
}

#[test]
fn unmatched_operation_from_another_project_does_not_pollute_selected_coverage() {
    let mut v = use_view(vec![use_operation("unassigned-mcp", "mcpTool", "failed")]);
    let project = "/synthetic/selected-project";
    v.projects.push(project.into());
    // This global inventory item applies to the selected project. The operation's
    // thread has no selected-project association, so it is outside that history scope.
    let mut request = uses_request();
    request.scope.project = Some(project.into());
    let result = execute(request, "config:uses".into(), &v).unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].observation, Observation::Unknown);
    assert!(!result.coverage.issues.iter().any(|issue| matches!(
        issue.code.as_str(),
        "usageOperationTargetUnknown" | "usageCountV4TargetUnknown"
    )));
}

#[test]
fn native_multiple_read_candidates_match_each_object_without_pretending_dispatch() {
    let mut native = use_operation("native", "command", "completed");
    native.path = None;
    native.work=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::WORK_OBSERVATION_VERSION,"stage":"terminal","data":{"kind":"command","cwd":"/synthetic","source":"agent","parsed_commands":[{"kind":"read","path":"SKILL.md"},{"kind":"read","path":"second/SKILL.md"},{"kind":"read","path":"./SKILL.md"}]},"gaps":[]})).unwrap());
    let mut v = use_view_paths(vec![native.clone(), native.clone()], true);
    let mut second = v.items[0].clone();
    second.id = "second".into();
    second.path = "/synthetic/second/SKILL.md".into();
    v.items.push(second);
    let result = execute(uses_request(), "config:multi".into(), &v).unwrap();
    assert_eq!(result.items.len(), 2);
    for item in &result.items {
        assert_eq!(item.usage_count, Some(0));
        assert_eq!(item.observation, Observation::Unknown);
        assert_eq!(item.counts.file_reads, 0);
        assert!(
            result
                .coverage
                .issues
                .iter()
                .any(|issue| issue.code == "usageCountV4DispatchUnknown"
                    && issue.path.as_deref() == Some(item.path.as_str()))
        );
    }
    let crate::adapters::contract::WorkData::Command { source, .. } =
        &mut native.work.as_mut().unwrap().data
    else {
        panic!()
    };
    *source = Some(crate::adapters::contract::CommandSource::UserShell);
    let user = use_view(vec![native]);
    let result = execute(uses_request(), "config:user-shell".into(), &user).unwrap();
    assert!(
        result
            .coverage
            .issues
            .iter()
            .all(|issue| !issue.code.starts_with("usageCount"))
    );
}
#[test]
fn canonical_replay_target_conflicts_do_not_depend_on_input_order() {
    let mut first = use_operation("canonical", "skillRead", "completed");
    first.path = Some("/synthetic/SKILL.md".into());
    let mut second = first.clone();
    second.path = Some("/synthetic/second/SKILL.md".into());
    for operations in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let mut v = use_view_paths(operations, true);
        let mut second = v.items[0].clone();
        second.id = "second".into();
        second.path = "/synthetic/second/SKILL.md".into();
        v.items.push(second);
        let result = execute(uses_request(), "config:conflict".into(), &v).unwrap();
        for item in result.items {
            assert_eq!(item.usage_count, Some(0));
        }
        assert!(
            result
                .coverage
                .issues
                .iter()
                .any(|issue| issue.code == "usageCountV4TargetUnknown")
        );
    }
}

#[test]
fn native_rule_read_display_label_is_not_loaded_or_an_executed_file_read() {
    let mut op = use_operation("native-rule", "command", "completed");
    op.work=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::WORK_OBSERVATION_VERSION,"stage":"terminal","data":{"kind":"command","cwd":"/synthetic","source":"agent","parsed_commands":[{"kind":"read","path":"AGENTS.md"}]},"gaps":[]})).unwrap());
    let mut v = use_view_paths(vec![op], true);
    v.items[0].kind = Kind::Rule;
    v.items[0].path = "/synthetic/AGENTS.md".into();
    let result = execute(uses_request(), "config:native-rule".into(), &v).unwrap();
    assert_eq!(result.items[0].counts.file_reads, 0);
    assert_eq!(result.items[0].observation, Observation::Unknown);
    assert!(
        result
            .coverage
            .issues
            .iter()
            .any(|issue| issue.code == "ruleReadDispatchUnknown")
    );
}

#[test]
fn replay_missing_time_retains_the_observed_window_count_in_both_orders() {
    let known = use_operation("canonical-time", "skillRead", "completed");
    let mut missing = known.clone();
    missing.timestamp = None;
    for operations in [
        vec![known.clone(), missing.clone()],
        vec![missing.clone(), known.clone()],
    ] {
        let v = use_view(operations);
        let all = execute(uses_request(), "config:time-replay".into(), &v).unwrap();
        assert_eq!(all.items[0].usage_count, Some(1));
        assert!(
            all.coverage
                .issues
                .iter()
                .any(|issue| issue.code == "usageCountV4TimeUnknown")
        );
        let request = Request {
            scope: Scope {
                since: Some("2026-10-04".into()),
                until: Some("2026-10-05".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let filtered = execute(request, "config:time-replay-window".into(), &v).unwrap();
        assert_eq!(filtered.items[0].usage_count, Some(1));
    }
}

#[test]
fn public_use_basis_binds_observed_or_partial_scalars_to_fixed_scope() {
    for (mut op, expected) in [
        (
            use_operation("known", "skillRead", "failed"),
            UseBasisStatus::Observed,
        ),
        (
            use_operation("candidate", "skillRead", "completed"),
            UseBasisStatus::Partial,
        ),
    ] {
        if expected == UseBasisStatus::Partial {
            op.name = "read_skill_file".into();
        }
        let v = use_view(vec![op]);
        let mut request = uses_request();
        request.scope.thread_id = Some("thread".into());
        let result = execute(request, "config:basis".into(), &v).unwrap();
        let item = &result.items[0];
        let basis = item.use_basis.as_ref().unwrap();
        assert_eq!(basis.method_version, 4);
        assert_eq!(basis.status, expected);
        assert_eq!(basis.captured_at, v.checked);
        assert_eq!(basis.snapshot_id, result.usage_revision);
        assert_eq!(basis.scope.source_instance_ids, ["source"]);
        assert_eq!(basis.scope.thread_id.as_deref(), Some("thread"));
        assert_eq!(basis.scope.window, UseWindow::AllHistory);
        assert_eq!(basis.time_basis, UseTimeBasis::SourceOperationTime);
        assert_eq!(basis.source_completeness, UseSourceCompleteness::Unknown);
        assert_eq!(
            basis.coverage.dispatch_gaps,
            Some(u64::from(expected == UseBasisStatus::Partial))
        );
        assert_eq!(
            item.usage_count,
            Some(u64::from(expected == UseBasisStatus::Observed))
        );
    }
}
#[test]
fn public_use_basis_preserves_known_zero_and_unavailable_is_not_zero_coverage() {
    let mut v = use_view(vec![]);
    let result = execute(uses_request(), "config:zero".into(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, Some(0));
    assert_eq!(
        result.items[0].use_basis.as_ref().unwrap().status,
        UseBasisStatus::Observed
    );
    v.snapshot = None;
    let result = execute(Request::default(), "config:unavailable".into(), &v).unwrap();
    assert_eq!(result.items[0].usage_count, None);
    let basis = result.items[0].use_basis.as_ref().unwrap();
    assert_eq!(basis.status, UseBasisStatus::Unavailable);
    assert_eq!(basis.coverage.identity_gaps, None);
    assert_eq!(
        basis.scope.window,
        UseWindow::DateWindow {
            since: "2026-09-02".into(),
            until: "2026-10-02".into(),
            timezone: "UTC".into()
        }
    );
}

#[test]
fn catalog_time_and_unselected_project_time_do_not_pollute_public_use_basis() {
    for kind in ["skillAvailable", "skillRead"] {
        let mut op = use_operation("outside", kind, "completed");
        op.timestamp = None;
        let mut v = use_view(vec![op]);
        Arc::get_mut(v.snapshot.as_mut().unwrap())
            .unwrap()
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
                status: "complete".into(),
                files_read: 1,
                bytes_read: 1,
                issues: vec![],
            });
        let mut request = Request::default();
        if kind == "skillRead" {
            let project = "/synthetic/selected";
            v.projects.push(project.into());
            request.scope.project = Some(project.into());
        }
        let result = execute(request, "config:scope".into(), &v).unwrap();
        let item = &result.items[0];
        assert_eq!(item.usage_count, Some(0));
        let basis = item.use_basis.as_ref().unwrap();
        assert_eq!(basis.coverage.time_gaps, Some(0));
        assert_eq!(basis.status, UseBasisStatus::Observed);
        assert_eq!(basis.source_completeness, UseSourceCompleteness::Complete);
    }
}

#[test]
fn unassigned_record_preserves_two_observed_canonical_reads_and_replay_deduplication() {
    let mut first = use_operation("first", "skillRead", "running");
    first.path = Some("/synthetic/SKILL.md".into());
    let mut result = first.clone();
    result.status = "completed".into();
    let mut second = use_operation("second", "skillRead", "failed");
    second.path = first.path.clone();
    let mut unassigned = use_operation("unassigned", "skillRead", "completed");
    unassigned.path = None;
    let v = use_view_paths(
        vec![first, result.clone(), result, second, unassigned],
        true,
    );
    let result = execute(uses_request(), "config:partial".into(), &v).unwrap();
    let item = &result.items[0];
    assert_eq!(item.usage_count, Some(2));
    assert_eq!(item.counts.file_reads, 2);
    assert_eq!(
        item.use_basis.as_ref().unwrap().status,
        UseBasisStatus::Partial
    );
    assert_eq!(
        item.use_basis.as_ref().unwrap().coverage.target_gaps,
        Some(1)
    );
}

#[test]
fn rule_reads_use_canonical_observed_count_with_partial_target_and_identity_coverage() {
    let path = "/synthetic/AGENTS.md";
    let mut operations = ["first", "second", "anonymous"].map(|id| {
        let mut op = use_operation(id, "tool", "completed");
        op.path = Some(path.into());
        op
    });
    operations[2].call_id = None;
    let mut unassigned = use_operation("unassigned", "tool", "completed");
    unassigned.path = None;
    let mut v = use_view_paths(
        vec![
            operations[0].clone(),
            operations[0].clone(),
            operations[1].clone(),
            operations[2].clone(),
            unassigned,
        ],
        true,
    );
    v.items[0].kind = Kind::Rule;
    v.items[0].path = path.into();
    let result = execute(uses_request(), "config:rule-partial".into(), &v).unwrap();
    let item = &result.items[0];
    assert_eq!(item.counts.file_reads, 2);
    assert_eq!(item.source_contexts[0].counts.file_reads, 2);
    let basis = item.use_basis.as_ref().unwrap();
    assert_eq!(basis.status, UseBasisStatus::Partial);
    assert_eq!(basis.coverage.identity_gaps, Some(1));
    assert_eq!(basis.coverage.target_gaps, Some(1));
}
