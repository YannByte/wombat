use super::*;
pub(super) fn files(root: &Path, contents: &[(&str, &str)]) -> Vec<Item> {
    let source = crate::adapters::contract::SourceInstance {
        id: "synthetic".into(),
        agent_kind: "codex".into(),
        root: root.to_string_lossy().into(),
    };
    for (path, text) in contents {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    crate::config::scan::scan(&[source], &[root.to_string_lossy().into()], "now").items
}
fn declaration(root: &Path, value: &str) {
    fs::create_dir_all(root.join(".wombat")).unwrap();
    fs::write(root.join(".wombat/analysis.json"), value).unwrap();
}
#[test]
fn exact_full_units_preserve_chinese_punctuation_case_and_original_positions() {
    let text = "# 要求\r\n\r\n先验证中文，保留 A。\r\n\r\n先验证中文，保留 A。\r\n\r\n先验证中文,保留 a。\r\n";
    let units = blocks(text);
    assert_eq!(units.len(), 3);
    assert_eq!(&text[units[0].range.clone()], &text[units[1].range.clone()]);
    assert_ne!(&text[units[1].range.clone()], &text[units[2].range.clone()]);
    assert_eq!(units[0].line, 3);
    assert_eq!(units[1].line, 5);
    assert_eq!(units[0].end_line, 3);
    assert_eq!(
        blocks("- 完整列表\n  - 子项\n\n- 完整列表\n  - 子项\n").len(),
        1
    );
    assert_eq!(
        blocks("> 示例\n\n```text\n示例\n```\n\n- 列表例子\n  ```text\n  示例\n  ```\n").len(),
        0
    );
}
#[test]
fn no_directory_inference_declared_chains_remain_distinct_from_actual_injection() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let projects = vec![root.to_string_lossy().into()];
    let items = files(
        &root,
        &[
            ("AGENTS.md", "相同完整中文指令。\n"),
            ("child/AGENTS.md", "相同完整中文指令。\n"),
        ],
    );
    assert!(analyze(&items, &projects).findings.is_empty());
    declaration(
        &root,
        r#"{"version":1,"chains":[{"id":"explicit","files":["AGENTS.md","child/AGENTS.md"]}]}"#,
    );
    let result = analyze(&items, &projects);
    assert_eq!(result.findings.len(), 2);
    for finding in result.findings.values().flatten() {
        let e = finding.evidence.as_ref().unwrap();
        assert_eq!(e.applicability, "declaredJointApplicability");
        assert_eq!(e.positions.len(), 2);
        assert_eq!(e.versions.len(), 2);
        assert!(
            finding
                .evidence_codes
                .contains(&"actualInjectionUnknown".into())
        );
    }
    let changed = files(
        &root,
        &[("child/AGENTS.md", "# 仅在删除时\n\n相同完整中文指令。\n")],
    );
    let result = analyze(&changed, &projects);
    assert!(result.findings.is_empty());
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.code == "blockApplicabilityUnknown")
    );
}
fn review(result: &Analysis, items: &[Item]) -> Suggestion {
    let (id, findings) = result.findings.iter().next().unwrap();
    Suggestion {
        review_format_version: 1,
        decision: None,
        baseline_id: None,
        checks: vec![],
        id: "review".into(),
        scope_project: None,
        item: items.iter().find(|i| &i.id == id).unwrap().clone(),
        category: Category::Trim,
        findings: findings.clone(),
        checked_at: "now".into(),
        rule_version: RuleParameters::default().version,
        rule_parameters: None,
    }
}
#[test]
fn recheck_requires_complete_owner_scoped_relation_and_all_members() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let other = root.join("other");
    fs::create_dir(&other).unwrap();
    let manifest =
        r#"{"version":1,"chains":[{"id":"joint","files":["AGENTS.md","child/AGENTS.md"]}]}"#;
    declaration(&root, manifest);
    declaration(&other, manifest);
    let items = files(
        &root,
        &[
            ("AGENTS.md", "# A\n\n相同完整指令。\n"),
            ("child/AGENTS.md", "# A\n\n相同完整指令。\n"),
        ],
    );
    let projects = vec![root.to_string_lossy().into()];
    let initial = analyze(&items, &projects);
    let review = review(&initial, &items);
    assert!(complete(&initial, &review));
    let changed = files(&root, &[("child/AGENTS.md", "# B\n\n相同完整指令。\n")]);
    let unknown = analyze(&changed, &projects);
    assert!(
        unknown
            .issues
            .iter()
            .any(|i| i.code == "blockApplicabilityUnknown")
    );
    assert!(!complete(&unknown, &review));
    fs::remove_file(root.join(".wombat/analysis.json")).unwrap();
    let mut all = items.clone();
    all.extend(files(
        &other,
        &[
            ("AGENTS.md", "独立指令一。\n"),
            ("child/AGENTS.md", "独立指令二。\n"),
        ],
    ));
    assert!(!complete(
        &analyze(
            &all,
            &[
                root.to_string_lossy().into(),
                other.to_string_lossy().into()
            ]
        ),
        &review
    ));
    declaration(&root, manifest);
    let restored = files(&root, &[("child/AGENTS.md", "# A\n\n相同完整指令。\n")]);
    assert!(complete(&analyze(&restored, &projects), &review));
    let mut incomplete = review.clone();
    incomplete.findings[0].evidence.as_mut().unwrap().relation = None;
    assert!(!complete(&initial, &incomplete));
    let mut incomplete = initial;
    let relation = review.findings[0]
        .evidence
        .as_ref()
        .unwrap()
        .relation
        .as_ref()
        .unwrap();
    let missing = match incomplete.relations.get(relation).unwrap() {
        RelationAssessment::Complete(ids) => ids.iter().last().unwrap().clone(),
        _ => unreachable!(),
    };
    incomplete.assessed.remove(&missing);
    assert!(!complete(&incomplete, &review));
}
#[test]
fn declared_copies_require_explicit_direction_and_supported_transform() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let projects = vec![root.to_string_lossy().into()];
    let items = files(
        &root,
        &[("AGENTS.md", "原件\n"), ("copy/AGENTS.md", "副本\n")],
    );
    assert!(analyze(&items, &projects).findings.is_empty());
    declaration(
        &root,
        r#"{"version":1,"copies":[{"id":"copy","source":"AGENTS.md","copy":"copy/AGENTS.md","transform":"identity-v1"}]}"#,
    );
    let result = analyze(&items, &projects);
    assert_eq!(result.findings.len(), 1);
    let f = result.findings.values().next().unwrap().first().unwrap();
    assert_eq!(f.rule, "declaredCopyDrift");
    let e = f.evidence.as_ref().unwrap();
    assert_eq!(e.direction.as_deref(), Some("sourceToCopy"));
    assert_eq!(e.versions.len(), 2);
    let revised = files(&root, &[("AGENTS.md", "另一原件\n")]);
    let latest = analyze(&revised, &projects);
    assert_ne!(
        serde_json::to_vec(&result.findings).unwrap(),
        serde_json::to_vec(&latest.findings).unwrap()
    );
    declaration(
        &root,
        r#"{"version":1,"copies":[{"id":"copy","source":"AGENTS.md","copy":"copy/AGENTS.md","transform":"unknown"}]}"#,
    );
    let result = analyze(&revised, &projects);
    assert!(result.findings.is_empty());
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.code == "copyTransformUnsupported")
    );
}
#[test]
fn short_repetitions_and_nested_list_overlap_do_not_replace_full_blocks() {
    let units = blocks(
        "保留原句。 后面不同一。\n\n保留原句。 后面不同二。\n\n- 顶层\n  - 中文子项\n  - 中文子项\n",
    );
    assert_eq!(units.len(), 3);
    assert_ne!(
        &"保留原句。 后面不同一。\n\n保留原句。 后面不同二。\n\n- 顶层\n  - 中文子项\n  - 中文子项\n"
            [units[0].range.clone()],
        &"保留原句。 后面不同一。\n\n保留原句。 后面不同二。\n\n- 顶层\n  - 中文子项\n  - 中文子项\n"
            [units[1].range.clone()]
    );
}
#[test]
fn heading_format_and_conditions_are_not_normalized_into_equivalence() {
    let units = blocks("# **删除** 时\n\n原文\n\n# **构建** 时\n\n原文\n");
    assert_eq!(units.len(), 2);
    assert_ne!(units[0].context.headings, units[1].context.headings);
}

fn complete(result: &Analysis, suggestion: &Suggestion) -> bool {
    suggestion
        .findings
        .iter()
        .all(|f| result.complete_finding(&suggestion.item.id, f))
}
