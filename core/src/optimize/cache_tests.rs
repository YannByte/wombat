//! Independent cache truths; no source collection, services or durable user records.
use super::*;
use std::sync::Mutex;
fn item(id: &str) -> Item {
    serde_json::from_value(serde_json::json!({
        "id": id, "name": id, "kind": "rule", "sourceInstanceId": "source", "path": "/safe/AGENTS.md",
        "authorizedProjects": [], "sourceContexts": [], "configuredState": "discovered", "contentHash": "v1",
        "observedAt": "2026-10-01T00:00:00Z", "current": true, "stale": false, "bytes": 10,
        "bytesSource": "completeUtf8File", "estimateStatus": "unknown", "measurementStatus": "complete",
        "bodyEstimateStatus": "unknown", "observation": "unknown", "counts": {"fileReads":0,"toolCalls":0,"resourceReads":0,"succeeded":0,"failed":0,"outcomeUnknown":0},
        "relatedTurns": 0,"relatedTasks": 0
    })).unwrap()
}
fn view() -> View {
    View {
        items: vec![item("object")],
        snapshot: None,
        issues: vec![],
        projects: vec![],
        roots: vec!["/safe".into()],
        project_roots: vec![],
        revision: "unused-global-version".into(),
        checked: "2026-10-01T00:00:00Z".into(),
        history_status: "unavailable".into(),
        analysis: Default::default(),
        hook_registry: Default::default(),
        config_collection: Default::default(),
        observation_versions: Default::default(),
    }
}
fn cached(
    rule: Rule,
    view: &View,
    rules: &RuleParameters,
    cache: &Mutex<cache::JudgmentCache>,
) -> Assessment {
    assess(
        rule,
        &Input::initial(view, &view.items[0], rules, None),
        cache,
    )
    .unwrap()
}
fn hits(cache: &Mutex<cache::JudgmentCache>) -> usize {
    cache.lock().unwrap().stats().2
}
fn finding(rule: &str) -> Finding {
    super::finding(
        Rule::FileSize,
        "test",
        Some(10),
        Some(20),
        vec![rule.into()],
    )
}
#[test]
fn cold_and_warm_results_use_current_metadata_and_ignore_unrelated_updates() {
    let cache = Mutex::default();
    let rules = RuleParameters::default();
    let mut view = view();
    let cold = evaluate_cached(&Input::initial(&view, &view.items[0], &rules, None), &cache);
    let warm = evaluate_cached(&Input::initial(&view, &view.items[0], &rules, None), &cache);
    assert_eq!(
        serde_json::to_value(&cold).unwrap(),
        serde_json::to_value(warm).unwrap()
    );
    let before = hits(&cache);
    view.checked = "2026-10-02T00:00:00Z".into();
    view.revision = "unrelated-log-revision".into();
    view.history_status = "fixed".into();
    view.items.push(item("unrelated"));
    view.items[1].content_hash = "unrelated-change".into();
    view.items[0].usage_count = Some(999);
    view.items[0].observed_at = view.checked.clone();
    let later = evaluate_cached(&Input::initial(&view, &view.items[0], &rules, None), &cache);
    assert!(hits(&cache) > before);
    assert!(
        later
            .iter()
            .all(|judgment| judgment.checked_at == view.checked)
    );
    let baseline: Vec<_> = cold
        .into_iter()
        .map(|mut assessment| {
            assessment.checked_at = view.checked.clone();
            assessment.basis.cutoff = view.checked.clone();
            assessment.assessment_id = None;
            super::super::identity::identify(&mut assessment);
            assessment
        })
        .collect();
    assert_eq!(
        serde_json::to_value(baseline).unwrap(),
        serde_json::to_value(later).unwrap()
    );
}
#[test]
fn actual_parameters_measurement_methods_and_content_changes_recompute() {
    let cache = Mutex::default();
    let mut rules = RuleParameters::default();
    let mut view = view();
    assert!(matches!(
        cached(Rule::FileSize, &view, &rules, &cache),
        Assessment::Miss
    ));
    rules.overrides.description_characters = Some(3);
    cached(Rule::FileSize, &view, &rules, &cache);
    assert_eq!(hits(&cache), 1); // This rule does not consume description parameters.
    rules.overrides.agents_bytes = Some(5);
    assert!(matches!(
        cached(Rule::FileSize, &view, &rules, &cache),
        Assessment::Hit(_)
    ));
    assert_eq!(hits(&cache), 1);
    view.items[0].bytes_source = Some("other-safe-measurement-v2".into());
    cached(Rule::FileSize, &view, &rules, &cache);
    assert_eq!(hits(&cache), 1);
    view.items[0].content_hash = "v2".into();
    cached(Rule::FileSize, &view, &rules, &cache);
    assert_eq!(hits(&cache), 1);
}
#[test]
fn cached_miss_rechecks_baseline_version_parameters_and_availability() {
    let cache = Mutex::default();
    let rules = RuleParameters::default();
    let mut view = view();
    view.items[0].bytes = Some(20000);
    let original = evaluate_cached(&Input::initial(&view, &view.items[0], &rules, None), &cache)
        .into_iter()
        .find(|c| c.rule == "fileSize")
        .unwrap();
    let findings = original.findings.clone();
    let scope = original.basis.scope.clone();
    view.items[0].bytes = Some(0);
    cached(Rule::FileSize, &view, &rules, &cache);
    let check = |baseline: &RuleAssessment| {
        evaluate_cached(
            &Input {
                baseline: Some(Baseline {
                    findings: &findings,
                    assessments: std::slice::from_ref(baseline),
                    scope: &scope,
                }),
                ..Input::initial(&view, &view.items[0], &rules, None)
            },
            &cache,
        )
        .remove(0)
    };
    let before = hits(&cache);
    let comparable = check(&original);
    assert_eq!(comparable.outcome, RuleOutcome::Miss);
    assert_eq!(comparable.comparison.status, ComparisonStatus::Comparable);
    let mut old = original.clone();
    old.rule_version = "older".into();
    assert_eq!(
        check(&old).comparison.status,
        ComparisonStatus::Incomparable
    );
    old = original.clone();
    if let RuleMeasurement::Numeric { threshold, .. } = &mut old.basis.measurement {
        *threshold = 1;
    }
    assert_eq!(
        check(&old).comparison.status,
        ComparisonStatus::Incomparable
    );
    old = original;
    old.assessment_id = None;
    old.identity_gap = Some("budget".into());
    assert_eq!(check(&old).comparison.status, ComparisonStatus::Unknown);
    assert_eq!(hits(&cache), before + 4);
    view.items[0].stale = true;
    assert!(matches!(
        cached(Rule::FileSize, &view, &rules, &cache),
        Assessment::Insufficient("currentVersionUnavailable")
    ));
    assert_eq!(hits(&cache), before + 4);
}
#[test]
fn description_suppression_binds_its_measurement_and_both_thresholds() {
    let cache = Mutex::default();
    let mut rules = RuleParameters::default();
    let mut view = view();
    view.items[0].kind = crate::config_dto::Kind::Skill;
    view.items[0].skill_metadata = Some(crate::config_dto::SkillMetadata {
        status: "parsed".into(),
        description_characters: Some(1500),
        issues: vec![],
        diagnostics: vec![],
    });
    assert!(matches!(
        cached(Rule::DescriptionSize, &view, &rules, &cache),
        Assessment::Miss
    ));
    rules.overrides.description_characters = Some(50);
    assert!(matches!(
        cached(Rule::DescriptionSize, &view, &rules, &cache),
        Assessment::Miss
    ));
    assert_eq!(hits(&cache), 0);
    view.items[0]
        .skill_metadata
        .as_mut()
        .unwrap()
        .description_characters = Some(1600);
    cached(Rule::DescriptionSize, &view, &rules, &cache);
    assert_eq!(hits(&cache), 0);
    rules.body_tokens = 1;
    cached(Rule::DescriptionSize, &view, &rules, &cache);
    assert_eq!(hits(&cache), 1);
}
#[test]
fn project_roots_sources_and_target_memberships_isolate_judgments() {
    let rules = RuleParameters::default();
    let cache = Mutex::default();
    let first = view();
    cached(Rule::FileSize, &first, &rules, &cache);
    for scope in 0..5 {
        let mut next = view();
        match scope {
            0 => next.roots.push("/another-source".into()),
            1 => next
                .config_collection
                .source_roots
                .push("/persistent-authorization".into()),
            2 => next.project_roots.push("/project".into()),
            3 => next.items[0].authorized_projects.push("/project".into()),
            _ => next.items[0].source_instance_id = "another-source".into(),
        }
        cached(Rule::FileSize, &next, &rules, &cache);
        assert_eq!(hits(&cache), 0);
    }
    let input = Input::initial(&first, &first.items[0], &rules, Some("/project"));
    assess(Rule::FileSize, &input, &cache).unwrap();
    assert_eq!(hits(&cache), 0);
}
#[test]
fn invalid_ranges_and_unknown_evidence_never_become_cached_misses() {
    let cache = Mutex::default();
    let rules = RuleParameters::default();
    let mut view = view();
    assert!(matches!(
        cached(Rule::LocalReference, &view, &rules, &cache),
        Assessment::Insufficient(_)
    ));
    assert_eq!(cache.lock().unwrap().stats().0, 0);
    let mut finding = finding("localReference");
    finding.rule = "localReference".into();
    finding.evidence = Some(StaticEvidence {
        method: "safe-analysis-v1".into(),
        applicability: "current".into(),
        declaration_hash: None,
        relation_id: None,
        direction: None,
        transform: None,
        versions: vec![],
        positions: vec![BlockPosition {
            item_id: "object".into(),
            start_byte: 9,
            end_byte: 3,
            start_line: 1,
            end_line: 1,
            block_hash: "safe".into(),
        }],
        relation: None,
        references: vec![],
        hook: None,
    });
    view.analysis
        .findings
        .insert("object".into(), vec![finding]);
    let input = Input::initial(&view, &view.items[0], &rules, None);
    assert!(matches!(
        assess(Rule::LocalReference, &input, &cache),
        Err(EvaluationError::InvalidAnalysisRange)
    ));
    assert_eq!(cache.lock().unwrap().stats().0, 0);
    view.analysis.findings.get_mut("object").unwrap()[0]
        .evidence
        .as_mut()
        .unwrap()
        .positions[0]
        .end_byte = 10;
    assert!(matches!(
        cached(Rule::LocalReference, &view, &rules, &cache),
        Assessment::Hit(_)
    ));
    assert_eq!(cache.lock().unwrap().stats().0, 1);
    view.analysis.findings.get_mut("object").unwrap()[0]
        .evidence
        .as_mut()
        .unwrap()
        .positions[0]
        .start_byte = 8;
    cached(Rule::LocalReference, &view, &rules, &cache);
    assert_eq!(hits(&cache), 0);
}
#[test]
fn oversized_key_skips_reuse_without_changing_a_known_judgment() {
    let cache = Mutex::default();
    let rules = RuleParameters::default();
    let mut view = view();
    view.roots = vec!["x".repeat(cache::MAX_ENTRY_BYTES)];
    let input = Input::initial(&view, &view.items[0], &rules, None);
    assert!(
        super::super::inputs::key(Rule::FileSize, &input, &prepare(Rule::FileSize, &input))
            .is_none()
    );
    assert!(matches!(
        assess(Rule::FileSize, &input, &cache).unwrap(),
        Assessment::Miss
    ));
    assert_eq!(cache.lock().unwrap().stats().0, 0);
}
