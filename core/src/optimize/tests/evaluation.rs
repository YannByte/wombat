use super::*;
use crate::{
    config_dto::{HookRegistryStatus, HookTrust},
    optimize::{
        detection::detect,
        evaluation::{self, Baseline, Input},
    },
};

fn check(v: &View, rules: &RuleParameters, rule: &str) -> RuleAssessment {
    evaluation::evaluate(&Input::initial(v, &v.items[0], rules, None))
        .into_iter()
        .find(|c| c.rule == rule)
        .unwrap()
}
fn clean_skill() -> View {
    let mut v = view();
    v.items[0].skill_metadata = Some(SkillMetadata {
        status: "parsed".into(),
        description_characters: Some(100),
        issues: vec![],
        diagnostics: vec![],
    });
    v.items[0].body_estimate_status = "estimated".into();
    v.items[0].body_token_estimate = Some(body_estimate(100));
    v
}
fn static_finding(rule: &str) -> Finding {
    Finding {
        identity: Default::default(),
        rule: rule.into(),
        status: "failed".into(),
        observed: None,
        threshold: None,
        evidence_codes: vec![],
        basis: Some("syntheticStaticTruth".into()),
        evidence: Some(StaticEvidence {
            method: "synthetic-v1".into(),
            applicability: "authorizedCurrentConfigurationOnly".into(),
            declaration_hash: None,
            relation_id: None,
            direction: None,
            transform: None,
            versions: vec![],
            positions: vec![BlockPosition {
                item_id: "object".into(),
                start_byte: 3,
                end_byte: 9,
                start_line: 2,
                end_line: 3,
                block_hash: "synthetic-block".into(),
            }],
            relation: None,
            references: vec![],
            hook: None,
        }),
    }
}
#[test]
fn measurements_distinguish_complete_missing_and_unknown_for_each_object_class() {
    let rules = RuleParameters::default();
    let mut v = clean_skill();
    for rule in [
        "skillFormat",
        "descriptionStandard",
        "descriptionSize",
        "bodyTokens",
    ] {
        assert_eq!(check(&v, &rules, rule).outcome, RuleOutcome::Miss);
    }
    v.items[0].skill_metadata = None;
    for rule in [
        "skillFormat",
        "descriptionStandard",
        "descriptionSize",
        "bodyTokens",
    ] {
        assert_eq!(check(&v, &rules, rule).outcome, RuleOutcome::Insufficient);
    }
    v.items[0].kind = Kind::Rule;
    v.items[0].bytes = Some(1);
    assert_eq!(
        check(&v, &rules, "missingInstruction").outcome,
        RuleOutcome::Miss
    );
    assert_eq!(check(&v, &rules, "fileSize").outcome, RuleOutcome::Miss);
    v.items[0].bytes = None;
    assert_eq!(
        check(&v, &rules, "fileSize").outcome,
        RuleOutcome::Insufficient
    );
    for kind in [Kind::Rule, Kind::Skill, Kind::Mcp, Kind::Hook] {
        v.items[0].kind = kind;
        v.items[0].stale = true;
        assert!(
            evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, None))
                .iter()
                .all(|c| c.outcome == RuleOutcome::Insufficient)
        );
    }
}
#[test]
fn metadata_size_is_independent_of_content_read_failures_and_runtime_support() {
    let rules = RuleParameters::default();
    let mut v = clean_skill();
    v.items[0].kind = Kind::Rule;
    v.items[0].configured_state = "unreadable".into();
    v.items[0].measurement_status = "unavailable".into();
    v.items[0].content_hash.clear();
    v.items[0].bytes_source = Some("filesystemMetadata".into());
    v.items[0].bytes = Some(20_000);
    let size = check(&v, &rules, "fileSize");
    assert_eq!(size.outcome, RuleOutcome::Hit);
    assert_eq!(size.findings[0].observed, Some(20_000));
    assert!(
        size.method_versions
            .iter()
            .any(|m| m.method == "measurementSource:filesystemMetadata")
    );
    assert_eq!(
        check(&v, &rules, "localReference").outcome,
        RuleOutcome::Insufficient
    );
    assert!(
        !evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, None))
            .iter()
            .any(|c| c.rule == "instructionSelection")
    );
    for bytes in [Some(0), Some(16_384), Some(16_385), None] {
        v.items[0].bytes = bytes;
        assert_eq!(
            check(&v, &rules, "fileSize").outcome,
            match bytes {
                Some(n) if n > 16_384 => RuleOutcome::Hit,
                Some(_) => RuleOutcome::Miss,
                None => RuleOutcome::Insufficient,
            }
        );
    }
    v.items[0].bytes = Some(20_000);
    for source in [None, Some("utf8Payload"), Some("unverified")] {
        v.items[0].bytes_source = source.map(str::to_owned);
        assert_eq!(
            check(&v, &rules, "fileSize").outcome,
            RuleOutcome::Insufficient
        );
    }
    v.items[0].bytes_source = Some("filesystemMetadata".into());
    v.items[0].stale = true;
    assert_eq!(
        check(&v, &rules, "fileSize").outcome,
        RuleOutcome::Insufficient
    );
}
#[test]
fn cached_static_hits_survive_partial_coverage_but_empty_partial_analysis_cannot_pass() {
    let mut v = clean_skill();
    let rules = RuleParameters::default();
    let cached = static_finding("localReference");
    v.analysis
        .findings
        .insert("object".into(), vec![cached.clone()]);
    assert_eq!(
        check(&v, &rules, "localReference").outcome,
        RuleOutcome::Hit
    );
    v.analysis.findings.clear();
    assert_eq!(
        check(&v, &rules, "localReference").outcome,
        RuleOutcome::Insufficient
    );
    v.analysis
        .reference_checks
        .entry("object".into())
        .or_default()
        .complete = true;
    assert_eq!(
        check(&v, &rules, "localReference").outcome,
        RuleOutcome::Miss
    );
    let original = check(&v, &rules, "localReference");
    let baseline = [cached];
    let recheck = |v: &View| {
        evaluation::evaluate(&Input {
            baseline: Some(Baseline {
                findings: &baseline,
                assessments: std::slice::from_ref(&original),
                scope: &original.basis.scope,
            }),
            ..Input::initial(v, &v.items[0], &rules, None)
        })
        .remove(0)
    };
    assert_eq!(recheck(&v).outcome, RuleOutcome::Miss);
    v.analysis
        .reference_checks
        .get_mut("object")
        .unwrap()
        .complete = false;
    assert_eq!(recheck(&v).outcome, RuleOutcome::Insufficient);
}
#[test]
fn invalid_static_intervals_fail_only_the_consuming_rule_and_keep_valid_hits() {
    let rules = RuleParameters::default();
    for broken_rule in [
        "localReference",
        "exactInstructionBlocks",
        "declaredCopyDrift",
    ] {
        for reversed_lines in [false, true] {
            let mut v = view();
            let mut broken = static_finding(broken_rule);
            let p = &mut broken.evidence.as_mut().unwrap().positions[0];
            if reversed_lines {
                p.end_line = p.start_line - 1;
            } else {
                p.end_byte = p.start_byte - 1;
            }
            v.analysis.findings.insert(
                "object".into(),
                vec![broken, static_finding("localReference")],
            );
            let failed = check(&v, &rules, broken_rule);
            assert_eq!(failed.outcome, RuleOutcome::Error);
            assert_eq!(failed.reason.as_deref(), Some("invalidAnalysisEvidence"));
            assert!(failed.findings.is_empty());
            assert_eq!(check(&v, &rules, "skillFormat").outcome, RuleOutcome::Hit);
            assert_eq!(
                check(&v, &rules, "descriptionSize").outcome,
                RuleOutcome::Hit
            );
            let s = detect(&v, &rules).remove(0);
            assert!(!s.findings.iter().any(|f| f.rule == broken_rule));
            assert!(
                s.checks
                    .iter()
                    .any(|c| c.rule == broken_rule && c.outcome == RuleOutcome::Error)
            );
        }
    }
    let mut v = clean_skill();
    let mut broken = static_finding("localReference");
    broken
        .evidence
        .as_mut()
        .unwrap()
        .references
        .push(ReferenceEvidence {
            target: "secret-source-path".into(),
            base_directory: "/synthetic".into(),
            expected_type: None,
            status: "missing".into(),
            start_byte: 10,
            end_byte: 9,
            start_line: 1,
            end_line: 1,
        });
    v.analysis.findings.insert("object".into(), vec![broken]);
    let failed = check(&v, &rules, "localReference");
    assert_eq!(failed.outcome, RuleOutcome::Error);
    assert!(
        !serde_json::to_string(&failed)
            .unwrap()
            .contains("secret-source-path")
    );
    assert!(detect(&v, &rules).is_empty());
}
fn hook_finding(project: &str) -> Finding {
    let mut f = static_finding("hookTarget");
    f.evidence.as_mut().unwrap().hook = Some(HookTargetEvidence {
        project: project.into(),
        native_key: "hook-key".into(),
        registration_hash: "registration".into(),
        host_version: "1".into(),
        trust: HookTrust::Trusted,
        target: "/synthetic/hook".into(),
        status: "missing".into(),
    });
    f
}
#[test]
fn hook_initial_checks_and_rechecks_use_the_same_authorized_original_project_scope() {
    if !cfg!(unix) {
        return;
    }
    let rules = RuleParameters::default();
    let mut v = view();
    v.items[0].kind = Kind::Hook;
    v.items[0].skill_metadata = None;
    v.hook_registry.status = HookRegistryStatus::Observed;
    v.analysis
        .hook_checks
        .insert(("object".into(), "/project".into()), true);
    v.analysis
        .hook_checks
        .insert(("object".into(), "/other".into()), true);
    v.analysis.findings.insert(
        "object".into(),
        vec![hook_finding("/project"), hook_finding("/other")],
    );
    let initial =
        evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, Some("/project"))).remove(0);
    assert_eq!(initial.outcome, RuleOutcome::Hit);
    assert_eq!(initial.findings.len(), 1);
    let original = initial.clone();
    let baseline = initial.findings;
    v.analysis
        .findings
        .insert("object".into(), vec![hook_finding("/other")]);
    let recheck = |v: &View| {
        evaluation::evaluate(&Input {
            baseline: Some(Baseline {
                findings: &baseline,
                assessments: std::slice::from_ref(&original),
                scope: &original.basis.scope,
            }),
            ..Input::initial(v, &v.items[0], &rules, None)
        })
        .remove(0)
    };
    assert_eq!(recheck(&v).outcome, RuleOutcome::Miss);
    v.analysis
        .hook_checks
        .insert(("object".into(), "/project".into()), false);
    assert_eq!(recheck(&v).outcome, RuleOutcome::Insufficient);
    v.hook_registry.status = HookRegistryStatus::Unavailable;
    assert_eq!(recheck(&v).outcome, RuleOutcome::Unsupported);
}
#[test]
fn unchanged_inputs_are_deterministic_and_static_suggestion_order_identity_is_preserved() {
    let rules = RuleParameters::default();
    let mut v = clean_skill();
    let expected = vec![
        static_finding("exactInstructionBlocks"),
        static_finding("localReference"),
    ];
    v.analysis
        .findings
        .insert("object".into(), expected.clone());
    let first = detect(&v, &rules).remove(0);
    let strip_identity = |findings: &[Finding]| {
        let mut value = serde_json::to_value(findings).unwrap();
        for finding in value.as_array_mut().unwrap() {
            finding.as_object_mut().unwrap().shift_remove("identity");
        }
        value
    };
    assert_eq!(strip_identity(&first.findings), strip_identity(&expected));
    assert!(
        first
            .findings
            .iter()
            .all(|f| f.identity.gap.as_deref() == Some("problemLocationContextUnavailable"))
    );
    let expected_id = crate::hash(
        serde_json::to_vec(&(
            &v.items[0].id,
            &v.items[0].content_hash,
            &rules.version,
            rules.agents_bytes_default,
            rules.description_characters_default,
            rules.body_tokens,
            rules.description_standard_max,
            strip_identity(&expected),
        ))
        .unwrap(),
    );
    assert_eq!(first.id, expected_id);
    let assessments = evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, None));
    assert_eq!(
        serde_json::to_value(&assessments).unwrap(),
        serde_json::to_value(evaluation::evaluate(&Input::initial(
            &v,
            &v.items[0],
            &rules,
            None
        )))
        .unwrap()
    );
    v.checked = "2026-10-02T00:00:00Z".into();
    v.revision = "unrelated-observation".into();
    assert_eq!(detect(&v, &rules)[0].id, first.id);
}
#[test]
fn parameter_or_rule_changes_are_new_checks_and_cannot_clear_the_original_baseline() {
    let v = view();
    let old = RuleParameters::default();
    let first = check(&v, &old, "descriptionSize");
    assert_eq!(first.outcome, RuleOutcome::Hit);
    let original = first.clone();
    let baseline = first.findings;
    let mut new = old.clone();
    new.overrides.description_characters = Some(1000);
    assert_eq!(
        check(&v, &new, "descriptionSize").outcome,
        RuleOutcome::Miss
    );
    for rules in [
        &new,
        &RuleParameters {
            version: "new-rule-version".into(),
            ..new.clone()
        },
    ] {
        let recheck = evaluation::evaluate(&Input {
            baseline: Some(Baseline {
                findings: &baseline,
                assessments: std::slice::from_ref(&original),
                scope: &original.basis.scope,
            }),
            ..Input::initial(&v, &v.items[0], rules, None)
        })
        .remove(0);
        assert_eq!(recheck.outcome, RuleOutcome::Miss);
        assert_eq!(recheck.comparison.status, ComparisonStatus::Incomparable);
        assert_eq!(
            recheck.comparison.reason.as_deref(),
            Some("ruleParametersOrMethodChanged")
        );
    }
}

#[test]
fn checks_entry_exposes_the_same_facts_as_suggestions_without_requiring_a_suggestion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reviews.sqlite3");
    let v = view();
    let suggestions = query(&path, &v, Request::default());
    let checks = query(
        &path,
        &v,
        Request {
            action: Action::Checks,
            item_id: Some("object".into()),
            ..Default::default()
        },
    );
    assert_eq!(
        serde_json::to_value(&checks.checks).unwrap(),
        serde_json::to_value(&suggestions.suggestions[0].checks).unwrap()
    );
    let v = clean_skill();
    assert!(detect(&v, &RuleParameters::default()).is_empty());
    let checks = query(
        &path,
        &v,
        Request {
            action: Action::Checks,
            ..Default::default()
        },
    );
    assert!(
        checks
            .checks
            .iter()
            .any(|c| c.rule == "skillFormat" && c.outcome == RuleOutcome::Miss)
    );
    assert!(!checks.checks.iter().any(|c| c.rule == "skillInactivity"));
}
