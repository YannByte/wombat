use super::*;
use crate::optimize::service::run;

fn measured() -> View {
    let mut v = view();
    let i = &mut v.items[0];
    i.kind = Kind::Rule;
    i.bytes = Some(20_000);
    i.bytes_source = Some("filesystemMetadata".into());
    i.content_hash = "v1".into();
    i.measurement_status = "complete".into();
    v
}
fn initial(v: &View, path: &Path) -> Response {
    run(Request::default(), "config:baseline".into(), v, None, path).unwrap()
}
fn selection(r: &Response) -> Request {
    Request {
        action: Action::Recheck,
        baseline: Some(Box::new(r.suggestions[0].clone())),
        baseline_id: r.suggestions[0].baseline_id.clone(),
        ..Default::default()
    }
}
#[test]
fn inspection_and_comparable_recheck_never_create_a_review_store() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("user-v1/reviews.sqlite3");
    let before = measured();
    let first = initial(&before, &path);
    assert!(!first.suggestions.is_empty());
    assert!(!path.exists());
    let mut after = measured();
    after.items[0].bytes = Some(10);
    after.items[0].content_hash = "v2".into();
    let result = run(
        selection(&first),
        "config:after".into(),
        &after,
        Some(&before),
        &path,
    )
    .unwrap();
    assert!(result.checks.iter().any(|c| c.rule == "fileSize"
        && c.outcome == RuleOutcome::Miss
        && c.comparison.status == ComparisonStatus::Comparable));
    assert!(!path.exists());
}
#[test]
fn baseline_rejects_changed_rules_scope_and_missing_original() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let before = measured();
    let first = initial(&before, &path);
    let mut changed = selection(&first);
    changed.rule_overrides = Some(RuleOverrides {
        agents_bytes: Some(100),
        ..Default::default()
    });
    assert!(run(changed, "new".into(), &before, Some(&before), &path).is_err());
    let mut changed = selection(&first);
    changed.source_instance_id = Some("different".into());
    assert!(run(changed, "new".into(), &before, Some(&before), &path).is_err());
    let mut missing = selection(&first);
    missing.baseline = None;
    assert!(run(missing, "new".into(), &before, None, &path).is_err());
    assert!(!path.exists());
}
#[test]
fn a_rule_that_no_longer_applies_is_unknown_rather_than_resolved() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let before = measured();
    let first = initial(&before, &path);
    let mut after = measured();
    after.items[0].kind = Kind::Mcp;
    let r = run(
        selection(&first),
        "new".into(),
        &after,
        Some(&before),
        &path,
    )
    .unwrap();
    assert_eq!(r.result_status, "partial");
    assert!(
        r.checks
            .iter()
            .all(|c| c.outcome == RuleOutcome::Insufficient
                && c.comparison.status == ComparisonStatus::Unknown)
    );
    assert!(!r.checks.is_empty());
}
#[test]
fn explicit_decisions_survive_reads_and_rechecks_without_additional_writes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let before = measured();
    let first = initial(&before, &path);
    let decide = Request {
        action: Action::Keep,
        suggestion_id: Some(first.suggestions[0].id.clone()),
        decision_reason: Some(DecisionReason::Necessary),
        decision_revision: Some("0".into()),
        ..Default::default()
    };
    run(decide, "new".into(), &before, None, &path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    assert!(initial(&before, &path).suggestions.is_empty());
    let r = run(
        selection(&first),
        "new".into(),
        &before,
        Some(&before),
        &path,
    )
    .unwrap();
    assert!(r.checks.iter().any(|c| c.outcome == RuleOutcome::Hit));
    assert_eq!(saved, std::fs::read(&path).unwrap());
    let mut changed = measured();
    changed.items[0].content_hash = "changed".into();
    assert!(!initial(&changed, &path).suggestions.is_empty());
}
#[test]
fn invalid_decisions_and_unknown_store_versions_do_not_repair_or_erase_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let v = measured();
    let first = initial(&v, &path);
    let invalid = Request {
        action: Action::Keep,
        suggestion_id: Some(first.suggestions[0].id.clone()),
        decision_reason: Some(DecisionReason::Necessary),
        ..Default::default()
    };
    assert!(run(invalid, "new".into(), &v, None, &path).is_err());
    assert!(!path.exists());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE retained(value TEXT);INSERT INTO retained VALUES('keep');PRAGMA user_version=999;").unwrap();
    drop(db);
    let bytes = std::fs::read(&path).unwrap();
    assert!(run(Request::default(), "new".into(), &v, None, &path).is_err());
    assert_eq!(bytes, std::fs::read(&path).unwrap());
}
#[test]
fn missing_evidence_without_positive_findings_remains_partial_and_catalog_is_current() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let mut v = measured();
    v.items[0].bytes = None;
    v.items[0].configured_state = "unknown".into();
    let r = initial(&v, &path);
    assert_eq!(r.result_status, "partial");
    assert!(r.rule_catalog.iter().all(|c| {
        ![
            "skillInactivity",
            "mcpInactivity",
            "mcpFault",
            "runtimeDuplicateInjection",
            "instructionSelection",
            "skillDependency",
        ]
        .contains(&c.rule.as_str())
    }));
    assert!(!path.exists());
}

#[test]
fn inspection_excludes_other_projects_before_evaluating_rules() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let mut v = measured();
    v.projects = vec!["/selected".into(), "/other".into()];
    v.items[0].project = Some("/other".into());
    v.items[0].authorized_projects = vec!["/other".into()];
    let r = run(
        Request {
            project: Some("/selected".into()),
            ..Default::default()
        },
        "config:scoped".into(),
        &v,
        None,
        &path,
    )
    .unwrap();
    assert!(r.suggestions.is_empty());
    assert_eq!(r.result_status, "complete");
    assert!(!path.exists());
}

#[test]
fn a_current_miss_without_original_problem_identity_remains_partial() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let mut before = measured();
    before.items[0].bytes = Some(10);
    let mut finding = Finding {
        identity: Default::default(),
        rule: "localReference".into(),
        status: "review".into(),
        observed: Some(2),
        threshold: None,
        evidence_codes: vec![],
        basis: None,
        evidence: None,
    };
    // No original positions: the current miss cannot prove removal of that problem.
    finding.evidence = Some(StaticEvidence {
        method: "pulldown-cmark-0.13.4/markdown-resources-v1".into(),
        applicability: "sameFileSameHeading".into(),
        declaration_hash: None,
        relation_id: None,
        direction: None,
        transform: None,
        versions: vec![],
        positions: vec![],
        relation: None,
        references: vec![],
        hook: None,
    });
    before
        .analysis
        .findings
        .insert(before.items[0].id.clone(), vec![finding]);
    before
        .analysis
        .reference_checks
        .entry("object".into())
        .or_default()
        .complete = true;
    let first = initial(&before, &path);
    assert!(!first.suggestions.is_empty());
    let mut after = measured();
    after.items[0].bytes = Some(10);
    after.items[0].content_hash = "v2".into();
    after
        .analysis
        .reference_checks
        .entry("object".into())
        .or_default()
        .complete = true;
    let r = run(
        selection(&first),
        "new".into(),
        &after,
        Some(&before),
        &path,
    )
    .unwrap();
    assert!(
        r.checks
            .iter()
            .any(|c| c.outcome == RuleOutcome::Miss
                && c.comparison.status == ComparisonStatus::Unknown),
        "{}",
        serde_json::to_string(&r.checks).unwrap()
    );
    assert_eq!(r.result_status, "partial");
}

#[test]
fn concurrent_decisions_use_one_revision_and_only_one_write_commits() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reviews.sqlite3");
    let seed = measured();
    let first = initial(&seed, &path);
    let keep = |s: &Response| Request {
        action: Action::Keep,
        suggestion_id: Some(s.suggestions[0].id.clone()),
        decision_reason: Some(DecisionReason::Necessary),
        decision_revision: Some(s.decision_revision.clone()),
        ..Default::default()
    };
    run(keep(&first), "seed".into(), &seed, None, &path).unwrap();
    let mut v = measured();
    v.items[0].content_hash = "v2".into();
    let current = initial(&v, &path);
    let request = keep(&current);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let request = request.clone();
            let gate = gate.clone();
            std::thread::spawn(move || {
                let mut v = measured();
                v.items[0].content_hash = "v2".into();
                gate.wait();
                run(request, "concurrent".into(), &v, None, &path)
            })
        })
        .collect();
    let outcomes: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    let failure = outcomes.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert_eq!(
        failure
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "DECISION_CHANGED"
    );
    let success = outcomes.iter().find_map(|r| r.as_ref().ok()).unwrap();
    assert_eq!(success.decision_revision, "2");
    assert!(success.suggestions.is_empty());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM review_events", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
