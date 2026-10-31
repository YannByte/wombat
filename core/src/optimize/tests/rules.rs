use super::*;
use crate::optimize::{
    detection::{detect, parameters},
    evaluation::{self, Input},
    registry,
};
#[test]
fn rule_outcomes_do_not_turn_absent_findings_into_global_success() {
    let mut v = view();
    let rules = RuleParameters::default();
    let checks = evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, None));
    let outcome = |rule: &str| {
        checks
            .iter()
            .find(|c| c.rule == rule)
            .unwrap()
            .outcome
            .clone()
    };
    assert_eq!(outcome("skillFormat"), RuleOutcome::Hit);
    assert_eq!(outcome("descriptionStandard"), RuleOutcome::Miss);
    assert_eq!(outcome("bodyTokens"), RuleOutcome::Insufficient);
    assert_eq!(outcome("localReference"), RuleOutcome::Insufficient);
    assert!(
        checks
            .iter()
            .all(|c| c.content_version == v.items[0].content_hash)
    );
    v.items[0].stale = true;
    assert!(
        evaluation::evaluate(&Input::initial(&v, &v.items[0], &rules, None))
            .iter()
            .all(|c| c.outcome == RuleOutcome::Insufficient)
    );
    let unique = registry::catalog()
        .into_iter()
        .map(|d| d.rule)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), registry::catalog().len());
}

#[test]
fn missing_instruction_is_informational_and_requires_current_existence_evidence() {
    let mut v = view();
    let item = &mut v.items[0];
    item.kind = Kind::Rule;
    item.configured_state = "missing".into();
    item.measurement_status = "missing".into();
    item.skill_metadata = None;
    item.bytes = None;
    item.content_hash.clear();
    let result = detect(&v, &RuleParameters::default());
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].findings.len(), 1);
    assert_eq!(result[0].findings[0].rule, "missingInstruction");
    assert_eq!(result[0].findings[0].status, "informational");
    assert_eq!(
        result[0].findings[0].basis.as_deref(),
        Some("authorizedExistenceCheck")
    );
    v.items[0].measurement_status = "unreadable".into();
    assert!(detect(&v, &RuleParameters::default()).is_empty());
    v.items[0].measurement_status = "missing".into();
    v.items[0].stale = true;
    assert!(detect(&v, &RuleParameters::default()).is_empty());
}

#[test]
fn thresholds_distinguish_body_metadata_standards_and_product_reminders() {
    let mut v = view();
    let item = &mut v.items[0];
    item.bytes = Some(100_000);
    item.skill_metadata = Some(SkillMetadata {
        status: "parsed".into(),
        description_characters: Some(500),
        issues: vec![],
        diagnostics: vec![],
    });
    item.body_estimate_status = "estimated".into();
    item.body_token_estimate = Some(body_estimate(4999));
    let rules = RuleParameters::default();
    assert!(
        detect(&v, &rules).is_empty(),
        "large full file alone never triggers Skill body reminder"
    );
    for (n, expected) in [
        (500, None),
        (501, Some("descriptionSize")),
        (1024, Some("descriptionSize")),
        (1025, Some("descriptionStandard")),
    ] {
        v.items[0]
            .skill_metadata
            .as_mut()
            .unwrap()
            .description_characters = Some(n);
        let result = detect(&v, &rules);
        assert_eq!(
            result.first().map(|s| s.findings[0].rule.as_str()),
            expected
        );
        if n == 1025 {
            assert_eq!(result[0].category, Category::Repair);
            assert_eq!(result[0].findings.len(), 1);
        }
    }
    v.items[0].body_token_estimate = Some(body_estimate(5000));
    let combined = detect(&v, &rules);
    assert_eq!(combined.len(), 1);
    assert_eq!(combined[0].findings.len(), 2);
    assert_eq!(combined[0].findings[1].rule, "bodyTokens");
    v.items[0]
        .skill_metadata
        .as_mut()
        .unwrap()
        .description_characters = Some(500);
    v.items[0].body_token_estimate.as_mut().unwrap().method = "unknown-model".into();
    assert!(
        detect(&v, &rules).is_empty(),
        "unverified methods never trigger numeric reminders"
    );
    v.items[0].kind = Kind::Rule;
    v.items[0].skill_metadata = None;
    for (n, expected) in [(16384, 0), (16385, 1)] {
        v.items[0].bytes = Some(n);
        assert_eq!(detect(&v, &rules).len(), expected);
    }
    let custom = parameters(Some(RuleOverrides {
        agents_bytes: Some(20000),
        description_characters: Some(1024),
    }))
    .unwrap();
    assert!(detect(&v, &custom).is_empty());
    v.items[0].kind = Kind::Skill;
    v.items[0].skill_metadata = Some(SkillMetadata {
        status: "parsed".into(),
        description_characters: Some(1025),
        issues: vec![],
        diagnostics: vec![],
    });
    assert_eq!(
        detect(&v, &custom)[0].findings[0].rule,
        "descriptionStandard",
        "product overrides never bypass the standard"
    );
    assert!(
        parameters(Some(RuleOverrides {
            description_characters: Some(1025),
            ..Default::default()
        }))
        .is_err()
    );
}
