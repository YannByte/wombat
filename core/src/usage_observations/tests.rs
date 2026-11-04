use super::*;

#[test]
fn historical_paths_use_recorded_grammar_on_every_host() {
    for (path, base, expected) in [
        (
            "/synthetic/a/../skill/SKILL.md",
            None,
            "/synthetic/skill/SKILL.md",
        ),
        (
            "../skill/SKILL.md",
            Some("/synthetic/project"),
            "/synthetic/skill/SKILL.md",
        ),
        (
            r"C:\synthetic\a\..\skill\SKILL.md",
            None,
            r"c:\synthetic\skill\SKILL.md",
        ),
        (
            "C:/synthetic/skill/SKILL.md",
            None,
            r"c:\synthetic\skill\SKILL.md",
        ),
        (
            r"..\skill\SKILL.md",
            Some(r"C:\synthetic\project"),
            r"c:\synthetic\skill\SKILL.md",
        ),
        (
            "../skill/SKILL.md",
            Some(r"C:\synthetic\project"),
            r"c:\synthetic\skill\SKILL.md",
        ),
        (
            r"\\server\share\project\..\skill\SKILL.md",
            None,
            r"\\server\share\skill\SKILL.md",
        ),
        (
            r"literal\name/SKILL.md",
            Some("/synthetic"),
            r"/synthetic/literal\name/SKILL.md",
        ),
        (
            "/synthetic/skill/SKILL.md",
            Some(r"C:\unrelated"),
            "/synthetic/skill/SKILL.md",
        ),
    ] {
        assert_eq!(
            normalized_path(path, base).as_deref(),
            Some(expected),
            "{path:?} {base:?}"
        );
        assert!(is_skill_file(expected));
    }
    for (path, base) in [
        ("", Some("/synthetic")),
        ("skill/SKILL.md", None),
        ("skill/SKILL.md", Some("relative")),
        ("skill/SKILL.md", Some("file:///synthetic")),
        (r"C:skill\SKILL.md", Some(r"C:\synthetic")),
        (r"\skill\SKILL.md", Some(r"C:\synthetic")),
    ] {
        assert_eq!(normalized_path(path, base), None, "{path:?} {base:?}");
    }
    assert!(is_skill_file(r"skill\SKILL.md"));
    assert!(!is_skill_file("/synthetic/NOT_SKILL.md"));
    assert!(!is_skill_file(r"/synthetic/literal\SKILL.md"));
}

fn operation(id: &str, kind: &str) -> Operation {
    Operation {
        text_result: None,
        id: id.into(),
        thread_id: "thread".into(),
        turn_id: Some("turn".into()),
        item_id: None,
        call_id: Some(id.into()),
        response_id: None,
        kind: kind.into(),
        name: "read_file".into(),
        sequence: 1,
        timestamp: Some("2026-10-04T00:00:00Z".into()),
        time_precision: "second".into(),
        status: "completed".into(),
        exit_code: None,
        outcome_conflict: false,
        duration_ms: None,
        path: Some("/synthetic/skill/SKILL.md".into()),
        work: None,
        matching: None,
        server: None,
        tool: None,
        evidence: vec![],
    }
}

#[test]
fn three_independent_reads_in_one_turn_count_three_uses_and_one_related_turn() {
    let mut projection = Projection::default();
    for id in ["read-1", "read-2", "read-3"] {
        projection.observe(&operation(id, "skillRead"));
    }
    assert_eq!(projection.count(false), Some(3));
    assert_eq!(projection.related_turns(), 1);
    assert_eq!(projection.related_tasks(), 1);
}

#[test]
fn canonical_start_result_and_explicit_fork_replays_count_once() {
    // Aliases and fork ancestry were reconciled by the adapter. The projection
    // receives the same canonical owner/ID, regardless of physical evidence.
    let mut start = operation("canonical", "skillRead");
    start.status = "running".into();
    let mut result = start.clone();
    result.status = "completed".into();
    let replay = result.clone();
    let mut projection = Projection::default();
    for record in [start, result, replay] {
        projection.observe(&record);
    }
    assert_eq!(projection.count(false), Some(1));
    assert_eq!(projection.related_turns(), 1);
}

#[test]
fn failure_cancellation_unknown_and_independent_retry_still_count() {
    let mut projection = Projection::default();
    for (id, status) in [
        ("call", "failed"),
        ("retry", "interrupted"),
        ("third", "unknown"),
    ] {
        let mut op = operation(id, "skillRead");
        op.status = status.into();
        projection.observe(&op);
    }
    assert_eq!(projection.count(false), Some(3));
}

#[test]
fn catalogs_declarations_discovery_and_conflicts_are_excluded() {
    for kind in [
        "skillAvailable",
        "skillCatalog",
        "skillUse",
        "mcpDiscovery",
        "mcpUnclassified",
        "mcpConflict",
        "command",
    ] {
        assert_eq!(use_kind(&operation("unused", kind)), None, "{kind}");
    }
    for (kind, expected) in [
        ("skillRead", UseKind::SkillRead),
        ("mcpTool", UseKind::McpTool),
        ("mcpResource", UseKind::McpResource),
    ] {
        assert_eq!(use_kind(&operation("used", kind)), Some(expected));
    }
    let mut read = operation("read", "tool");
    assert_eq!(use_kind(&read), Some(UseKind::SkillRead));
    read.path = Some("/synthetic/skill".into());
    assert_eq!(use_kind(&read), None);
    read.path = Some("/synthetic/skill/SKILL.md.backup".into());
    assert_eq!(use_kind(&read), None);
}

#[test]
fn one_operation_can_be_projected_once_for_each_explicit_target() {
    let op = operation("two-target-read", "skillRead");
    let mut first = Projection::default();
    let mut second = Projection::default();
    first.observe(&op);
    second.observe(&op);
    assert_eq!(
        (first.count(false), second.count(false)),
        (Some(1), Some(1))
    );
}

#[test]
fn same_native_identity_in_an_independent_task_is_not_a_fork_replay() {
    let mut first = operation("canonical", "skillRead");
    let mut second = first.clone();
    second.thread_id = "independent-thread".into();
    // A missing link establishes no synthetic related turn.
    first.turn_id = None;
    second.turn_id = None;
    let mut projection = Projection::default();
    projection.observe(&first);
    projection.observe(&second);
    assert_eq!(projection.count(false), Some(2));
    assert_eq!(projection.related_turns(), 0);
    assert_eq!(projection.related_tasks(), 2);
    assert_eq!(projection.coverage.turn_gaps, 2);
}

#[test]
fn anonymous_reads_preserve_count_gaps_instead_of_promoting_hashes() {
    let mut op = operation("physical-record-hash", "skillRead");
    op.call_id = None;
    let mut projection = Projection::default();
    projection.observe(&op);
    assert_eq!(projection.count(false), None);
    assert_eq!(projection.coverage.identity_gaps, 1);
    assert_eq!(projection.related_turns(), 1);
}

#[test]
fn wrapper_literals_are_candidates_without_dispatch_evidence() {
    let mut op = operation("physical-record-hash", "skillRead");
    op.call_id = None;
    op.name = "read_skill_file".into();
    assert!(is_skill_read_candidate(&op));
    assert_eq!(use_kind(&op), None);
    let mut projection = Projection::default();
    projection.coverage.dispatch_gaps += 1;
    assert_eq!(projection.count(false), None);
    assert_eq!(projection.related_turns(), 0);
}

#[test]
fn missing_times_only_block_date_windows_while_target_gaps_always_block_counts() {
    for timestamp in [None, Some("invalid".into())] {
        let mut op = operation("known-call", "skillRead");
        op.timestamp = timestamp;
        assert_eq!(time_basis(&op), TimeBasis::Unknown);
        let mut projection = Projection::default();
        projection.observe(&op);
        assert_eq!(projection.count(false), Some(1));
        assert_eq!(projection.count(true), None);
        assert_eq!(projection.coverage.time_gaps, 1);
    }
    let mut projection = Projection::default();
    projection.observe(&operation("known-call", "skillRead"));
    projection.coverage.target_gaps += 1;
    assert_eq!(projection.count(false), None);
}

#[test]
fn reliable_operation_identity_requires_canonical_owner_and_native_call_or_item() {
    let base = operation("canonical", "skillRead");
    assert_eq!(operation_identity(&base), Some(("thread", "canonical")));
    let mut native_item = base.clone();
    native_item.call_id = None;
    native_item.item_id = Some("native-item".into());
    assert_eq!(
        operation_identity(&native_item),
        Some(("thread", "canonical"))
    );
    for gap in ["owner", "canonical", "native"] {
        let mut incomplete = base.clone();
        match gap {
            "owner" => incomplete.thread_id = "".into(),
            "canonical" => incomplete.id.clear(),
            _ => {
                incomplete.call_id = Some(String::new());
                incomplete.item_id = Some(String::new());
            }
        }
        assert_eq!(operation_identity(&incomplete), None, "{gap}");
        let mut projection = Projection::default();
        projection.observe(&incomplete);
        assert_eq!(projection.count(true), None, "{gap}");
        assert_eq!(projection.coverage.identity_gaps, 1);
    }
}

fn native_read(paths: &[Option<&str>]) -> Operation {
    let mut op = operation("native", "command");
    op.path = None;
    op.work = Some(crate::adapters::contract::WorkObservation {
        format_version: crate::adapters::contract::WORK_OBSERVATION_VERSION,
        stage: crate::adapters::contract::WorkStage::Terminal,
        data: crate::adapters::contract::WorkData::Command {
            cwd: Some("/synthetic/native-cwd".into()),
            source: Some(crate::adapters::contract::CommandSource::Agent),
            parsed_commands: Some(
                paths
                    .iter()
                    .map(|path| crate::adapters::contract::ParsedCommand::Read {
                        path: path.map(str::to_owned),
                    })
                    .collect(),
            ),
        },
        gaps: vec![],
    });
    op
}
#[test]
fn native_multiple_read_labels_remain_candidates_with_recorded_cwd_not_project() {
    let op = native_read(&[
        Some("a/SKILL.md"),
        Some("b/SKILL.md"),
        Some("a/SKILL.md"),
        None,
    ]);
    let targets = read_targets(&op).unwrap();
    assert!(targets.candidate && targets.unbound && targets.may_be_skill());
    assert_eq!(use_kind(&op), None);
    assert_eq!(
        targets.resolve(targets.paths[0], Some("/wrong-project")),
        Some("/synthetic/native-cwd/a/SKILL.md".into())
    );
    let mut op = op;
    let WorkData::Command { source, .. } = &mut op.work.as_mut().unwrap().data else {
        panic!()
    };
    *source = Some(CommandSource::UserShell);
    assert!(read_targets(&op).is_none());
    op.kind = "skillRead".into();
    assert_eq!(use_kind(&op), None);
    let WorkData::Command { source, cwd, .. } = &mut op.work.as_mut().unwrap().data else {
        panic!()
    };
    *source = Some(CommandSource::Agent);
    *cwd = Some("file:///synthetic/native-cwd".into());
    assert_eq!(
        read_targets(&op)
            .unwrap()
            .resolve("a/SKILL.md", Some("/wrong-project")),
        None
    );
}
#[test]
fn replay_target_conflicts_are_order_independent_and_not_based_on_outcome_or_time() {
    let first = operation("canonical", "skillRead");
    let mut second = first.clone();
    second.path = Some("/synthetic/other/SKILL.md".into());
    for records in [[&first, &second], [&second, &first]] {
        assert!(
            target_conflicts(|| records.into_iter(), |_| None, |_| true)
                .contains(&("thread", "canonical"))
        );
    }
    let mut result = first.clone();
    result.status = "failed".into();
    result.timestamp = None;
    assert!(target_conflicts(|| [&first, &result].into_iter(), |_| None, |_| true).is_empty());
    let mut anonymous = second;
    anonymous.call_id = None;
    assert!(target_conflicts(|| [&first, &anonymous].into_iter(), |_| None, |_| true).is_empty());
}

#[test]
fn scoped_replay_registry_checks_off_target_aliases_without_retaining_unrelated_identities() {
    let selected = operation("selected", "skillRead");
    let mut alias = selected.clone();
    alias.path = Some("/other/SKILL.md".into());
    let unrelated = operation("unrelated", "skillRead");
    let mut unrelated_alias = unrelated.clone();
    unrelated_alias.path = Some("/different/SKILL.md".into());
    let records = [&selected, &alias, &unrelated, &unrelated_alias];
    let conflicts = target_conflicts(
        || records.into_iter(),
        |_| None,
        |op| op.id == "selected" && op.path == selected.path,
    );
    assert_eq!(conflicts, BTreeSet::from([("thread", "selected")]));
}

#[test]
fn public_basis_source_completeness_uses_selected_reports_and_never_zeros_missing_coverage() {
    use crate::config_dto::*;
    let scope = UseScope {
        source_instance_ids: vec!["selected".into()],
        project: None,
        thread_id: None,
        agent_kind: None,
        window: UseWindow::AllHistory,
    };
    let temp = tempfile::tempdir().unwrap();
    let mut snapshot = crate::usage_store::memory(
        Default::default(),
        "live:basis".into(),
        crate::pricing_sync::current_at(temp.path()).unwrap(),
        None,
    )
    .unwrap();
    let report = |id: &str, status: &str| crate::adapters::contract::SourceReport {
        source: crate::adapters::contract::SourceInstance {
            id: id.into(),
            agent_kind: "codex".into(),
            root: "/synthetic".into(),
        },
        adapter_version: "synthetic".into(),
        source_versions: vec![],
        capabilities: Default::default(),
        status: status.into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    snapshot.manifest.sources = vec![
        report("selected", "complete"),
        report("unrelated", "failed"),
    ];
    let projection = Projection::default();
    for (status, expected) in [
        ("complete", UseSourceCompleteness::Complete),
        ("partial", UseSourceCompleteness::Partial),
    ] {
        snapshot.manifest.sources[0].status = status.into();
        let value = basis(
            Some(&projection),
            UseUnit::ObjectUse,
            scope.clone(),
            "2026-10-01T00:00:00Z",
            Some(&snapshot),
            false,
        );
        assert_eq!(value.source_completeness, expected);
        assert_eq!(value.status, UseBasisStatus::Observed);
        assert_eq!(value.coverage.target_gaps, Some(0));
    }
    let value = basis(
        None,
        UseUnit::ObjectUse,
        scope,
        "2026-10-01T00:00:00Z",
        None,
        false,
    );
    assert_eq!(value.status, UseBasisStatus::Unavailable);
    assert_eq!(value.coverage.dispatch_gaps, None);
    assert_eq!(value.source_completeness, UseSourceCompleteness::Unknown);
}

#[test]
fn partial_projection_preserves_observed_canonical_uses_and_known_zero() {
    use crate::config_dto::*;
    let scope = UseScope {
        source_instance_ids: vec!["source".into()],
        project: None,
        thread_id: None,
        agent_kind: None,
        window: UseWindow::AllHistory,
    };
    for count in [0, 2] {
        let mut projection = Projection::default();
        for id in ["first", "second"].into_iter().take(count) {
            let call = operation(id, "skillRead");
            projection.observe(&call);
            projection.observe(&call);
        }
        projection.coverage.target_gaps = 1;
        assert_eq!(projection.count(false), None);
        assert_eq!(projection.observed_count(), count as u64);
        let value = basis(
            Some(&projection),
            UseUnit::ObjectUse,
            scope.clone(),
            "2026-10-04T00:00:00Z",
            None,
            false,
        );
        assert_eq!(value.method_version, 4);
        assert_eq!(value.status, UseBasisStatus::Partial);
        assert_eq!(serde_json::to_value(value).unwrap()["status"], "partial");
    }
}
