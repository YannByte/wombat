use super::*;
fn source(root: &Path) -> SourceInstance {
    SourceInstance {
        id: "source".into(),
        agent_kind: "codex".into(),
        root: root.to_string_lossy().into_owned(),
    }
}

#[test]
fn skill_queries_do_not_walk_unrelated_project_files_or_starve_known_skills() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let source_root = root.join("source");
    let project = root.join("project");
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(project.join(".agents/skills/known")).unwrap();
    fs::write(
        project.join(".agents/skills/known/SKILL.md"),
        "---\nname: known\ndescription: Synthetic\n---\n",
    )
    .unwrap();
    let clutter = project.join("unrelated");
    fs::create_dir(&clutter).unwrap();
    for i in 0..ENTRY_LIMIT + 1 {
        fs::write(clutter.join(format!("{i}.txt")), "").unwrap();
    }
    let projects = vec![project.to_string_lossy().into_owned()];
    let selected = scan_selected(&[source(&source_root)], &projects, "now", &[Kind::Skill]);
    assert_eq!(selected.items.len(), 1);
    assert!(selected.examined < 20);
    assert!(selected.issues.is_empty());
    let combined = scan(&[source(&source_root)], &projects, "now");
    assert!(
        combined
            .items
            .iter()
            .any(|item| item.kind == Kind::Skill && item.name == "known")
    );
    assert!(
        combined
            .issues
            .iter()
            .any(|issue| issue.code == "resourceLimited")
    );
}
#[test]
fn hook_declarations_are_discovered_without_executing_or_claiming_effectiveness() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    fs::write(root.join("hooks.json"),r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"SYNTHETIC_SECRET_COMMAND"}]}]}}"#).unwrap();
    fs::write(root.join("config.toml"),"[[hooks.PreToolUse]]\nmatcher='Bash'\n[[hooks.PreToolUse.hooks]]\ntype='command'\ncommand='SYNTHETIC_SECRET_INLINE'\n").unwrap();
    let result = scan(&[source(&root)], &[], "now");
    let hooks = result
        .items
        .iter()
        .filter(|item| item.kind == Kind::Hook)
        .collect::<Vec<_>>();
    assert_eq!(hooks.len(), 2);
    assert!(hooks.iter().all(|i| i.configured_state == "declared"
        && i.usage_count.is_none()
        && i.content_tokens.is_some()
        && i.observation == Observation::Unknown));
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.code == "hookEffectiveRegistryUnavailable")
    );
    assert!(
        !serde_json::to_string(&result.items)
            .unwrap()
            .contains("SYNTHETIC_SECRET")
    );
}
#[test]
fn unavailable_source_directory_cannot_prove_a_missing_instruction_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("absent");
    let result = scan(&[source(&root)], &[], "now");
    assert!(result.items.is_empty());
    assert!(result.issues.iter().any(|i| i.code == "configUnreadable"));
}
#[test]
fn inventory_only_exposes_metadata_and_never_runs_commands() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(root.join("skills/review")).unwrap();
    fs::write(root.join("skills/review/SKILL.md"), "合成规则").unwrap();
    fs::write(root.join("config.toml"), "[mcp_servers.example]\ncommand = 'SYNTHETIC_SECRET_COMMAND'\nenv = { TOKEN = 'SYNTHETIC_SECRET_VALUE' }\nenabled = false\n").unwrap();
    let result = scan(&[source(&root)], &[], "now");
    assert_eq!(result.items.len(), 3);
    let skill = result.items.iter().find(|i| i.kind == Kind::Skill).unwrap();
    assert_eq!(skill.bytes, Some("合成规则".len() as u64));
    assert!(skill.content_tokens.is_some());
    assert_eq!(skill.characters, Some(4));
    assert_eq!(
        skill.estimate.as_ref().unwrap().content_hash,
        skill.content_hash
    );
    assert_eq!(skill.observation, Observation::Unknown);
    let mcp = result.items.iter().find(|i| i.kind == Kind::Mcp).unwrap();
    assert_eq!(mcp.configured_state, "disabled");
    assert_eq!(mcp.bytes, None);
    let text = serde_json::to_string(&result.items).unwrap();
    assert!(!text.contains("SYNTHETIC_SECRET"));
    assert!(!text.contains("合成规则"));
}
#[test]
fn malformed_and_oversized_files_are_explicit_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    fs::write(root.join("config.toml"), "[broken").unwrap();
    fs::write(root.join("AGENTS.md"), vec![b'x'; FILE_LIMIT as usize + 1]).unwrap();
    let result = scan(&[source(&root)], &[], "now");
    assert_eq!(result.items.len(), 1);
    let row = &result.items[0];
    assert_eq!(row.bytes, Some(FILE_LIMIT + 1));
    assert_eq!(row.bytes_source.as_deref(), Some("filesystemMetadata"));
    assert_eq!(row.measurement_status, "unavailable");
    assert!(
        row.characters.is_none() && row.content_tokens.is_none() && row.content_hash.is_empty()
    );
    assert!(result.issues.iter().any(|i| i.code == "configInvalid"));
    assert!(result.issues.iter().any(|i| i.code == "resourceLimited"));
}
#[test]
fn shared_projects_scan_once_and_keep_global_scope_specific_to_each_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let project = root.join("project");
    let second_root = root.join("second");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&second_root).unwrap();
    let text = "合成当前完整规则。\n";
    fs::write(project.join("AGENTS.md"), text).unwrap();
    let first = SourceInstance {
        id: "first".into(),
        agent_kind: "codex".into(),
        root: project.to_string_lossy().into(),
    };
    let second = SourceInstance {
        id: "second".into(),
        agent_kind: "codex".into(),
        root: second_root.to_string_lossy().into(),
    };
    let projects = vec![project.to_string_lossy().into_owned()];
    let single = scan(std::slice::from_ref(&second), &projects, "now");
    let both = scan(&[second.clone(), first.clone()], &projects, "now");
    let object = both
        .items
        .iter()
        .find(|i| i.path == project.join("AGENTS.md").to_string_lossy())
        .unwrap();
    assert_eq!(object.source_contexts.len(), 2);
    assert!(object.applies(Some("first"), Some("/independent")));
    assert!(!object.applies(Some("second"), Some("/independent")));
    assert!(object.applies(Some("second"), Some(&projects[0])));
    let reverse = scan(&[first, second], &projects, "now");
    let same = reverse.items.iter().find(|i| i.id == object.id).unwrap();
    assert_eq!(same.source_instance_id, object.source_instance_id);
    assert_eq!(
        serde_json::to_value(&same.source_contexts).unwrap(),
        serde_json::to_value(&object.source_contexts).unwrap()
    );
    assert_eq!(single.used, text.len() as u64);
    // One global read plus one shared project read; no project read per source.
    assert_eq!(both.used, 2 * text.len() as u64);
    for c in &object.source_contexts {
        assert!(!c.inventory_id.is_empty());
    }
}
#[test]
fn overlapping_roots_keep_one_object_with_both_authorized_memberships() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let project = root.join("project");
    let child = project.join("child");
    fs::create_dir_all(&child).unwrap();
    fs::write(child.join("AGENTS.md"), "完整合成指令。\n").unwrap();
    let roots = vec![
        project.to_string_lossy().into_owned(),
        child.to_string_lossy().into_owned(),
    ];
    let result = scan(&[source(&root)], &roots, "now");
    let found: Vec<_> = result
        .items
        .iter()
        .filter(|i| i.path == child.join("AGENTS.md").to_string_lossy())
        .collect();
    assert_eq!(found.len(), 1);
    let item = found[0];
    assert_eq!(item.project.as_ref(), Some(&roots[1]));
    assert_eq!(item.authorized_projects, roots);
    assert!(item.applies(None, Some(&roots[0])) && item.applies(None, Some(&roots[1])));
    assert!(!item.applies(None, Some("/unauthorized")));
    let reverse = scan(
        &[source(&root)],
        &[roots[1].clone(), roots[0].clone()],
        "now",
    );
    let same = reverse.items.iter().find(|i| i.id == item.id).unwrap();
    assert_eq!(same.authorized_projects, item.authorized_projects);
    assert_eq!(same.project, item.project);
}
#[test]
fn recursive_agents_exact_names_and_measurement_states() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let project = root.join("project");
    fs::create_dir_all(project.join("nested")).unwrap();
    fs::create_dir_all(project.join("node_modules/example")).unwrap();
    fs::write(project.join("AGENTS.md"), "").unwrap();
    fs::write(project.join("nested/AGENTS.md"), "你好\r\n").unwrap();
    fs::create_dir(project.join("lowercase")).unwrap();
    fs::write(project.join("lowercase/agents.md"), "wrong").unwrap();
    fs::write(project.join("nested/AGENTS.override.md"), "wrong").unwrap();
    fs::write(project.join("node_modules/example/AGENTS.md"), "excluded").unwrap();
    let result = scan(&[source(&root)], &[project.to_string_lossy().into()], "now");
    let rows = result
        .items
        .iter()
        .filter(|i| i.project.is_some())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    let empty = rows
        .iter()
        .find(|i| i.path == project.join("AGENTS.md").to_string_lossy())
        .unwrap();
    assert_eq!(empty.bytes, Some(0));
    assert_eq!(empty.characters, Some(0));
    assert_eq!(empty.content_tokens, Some(0));
    assert_eq!(empty.measurement_status, "complete");
    let unicode = rows
        .iter()
        .find(|i| Path::new(&i.path).ends_with(Path::new("nested").join("AGENTS.md")))
        .unwrap();
    assert_eq!(unicode.bytes, Some(8));
    assert_eq!(unicode.characters, Some(4));
    fs::write(project.join("AGENTS.md"), [255, 254]).unwrap();
    let invalid = scan(&[source(&root)], &[project.to_string_lossy().into()], "now");
    let invalid = invalid.items.iter().find(|i| i.path == empty.path).unwrap();
    assert_eq!(invalid.bytes, Some(2));
    assert!(
        invalid.characters.is_none()
            && invalid.content_tokens.is_none()
            && invalid.content_hash.is_empty()
    );
    fs::remove_file(project.join("AGENTS.md")).unwrap();
    let missing = scan(&[source(&root)], &[project.to_string_lossy().into()], "now");
    let missing = missing.items.iter().find(|i| i.path == empty.path).unwrap();
    assert_eq!(missing.measurement_status, "missing");
    assert_eq!(missing.bytes, None);
}
#[cfg(unix)]
#[test]
fn symlinks_cannot_escape_the_authorized_root_or_recurse_forever() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("AGENTS.md"), "private").unwrap();
    std::os::unix::fs::symlink(outside.path().join("AGENTS.md"), root.join("AGENTS.md")).unwrap();
    fs::create_dir(root.join("skills")).unwrap();
    std::os::unix::fs::symlink(root.join("skills"), root.join("skills/loop")).unwrap();
    let result = scan(&[source(&root)], &[], "now");
    assert_eq!(result.items.len(), 1);
    assert!(result.items[0].bytes.is_none());
    assert!(result.items[0].content_hash.is_empty());
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.code == "outsideAuthorizedRoot")
    );
}
#[test]
fn hooks_measure_only_exact_declarations_and_ignore_state_tables() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let json_handler = r#"{ "type": "command", "command": "python3 '工具/large.py'" }"#;
    fs::create_dir(root.join("工具")).unwrap();
    fs::write(root.join("工具/large.py"), "x".repeat(2_000_000)).unwrap();
    fs::write(root.join("hooks.json"), format!(r#"{{"description":"not a handler", "hooks":{{"SessionStart":[{{"matcher":"startup","hooks":[{json_handler}]}}]}}}}"#)).unwrap();
    let toml_handler = "type = 'command'\ncommand = 'node script.js'\n";
    fs::write(root.join("config.toml"), format!("model='synthetic'\n[[hooks.PreToolUse]]\nmatcher='Bash'\n[[hooks.PreToolUse.hooks]]\n{toml_handler}[hooks.state.'synthetic-key']\nenabled=false\ntrusted_hash='synthetic'\n")).unwrap();
    let result = scan(&[source(&root)], &[], "now");
    assert!(!result.issues.iter().any(|i| i.code == "hookConfigInvalid"));
    let rows: Vec<_> = result
        .items
        .iter()
        .filter(|i| i.kind == Kind::Hook)
        .collect();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let expected = if row.path.ends_with("hooks.json") {
            json_handler
        } else {
            "[[hooks.PreToolUse.hooks]]\ntype = 'command'\ncommand = 'node script.js'"
        };
        assert_eq!(row.bytes, Some(expected.len() as u64), "{}", row.path);
        assert_eq!(row.characters, Some(expected.chars().count() as u64));
        assert_eq!(
            row.estimate.as_ref().unwrap().content_hash,
            crate::hash(expected)
        );
        assert_eq!(row.estimate.as_ref().unwrap().payload, "hookDeclaration");
        assert_eq!(
            row.estimate.as_ref().unwrap().tokens,
            crate::config::measure::estimate(expected, &crate::hash(expected))
                .unwrap()
                .tokens
        );
        assert_eq!(row.configured_state, "declared");
        assert!(row.usage_count.is_none());
    }
}
#[test]
fn hook_measurements_preserve_inline_toml_and_escaped_json_text() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let inline = r#"{type="command", command="node 中文.js"}"#;
    fs::write(
        root.join("config.toml"),
        format!("[hooks]\nPreToolUse=[{{matcher='.*', hooks=[{inline}]}}]\n"),
    )
    .unwrap();
    let handler = r#"{"type":"command","command":"node \u4e2d.js"}"#;
    fs::write(
        root.join("hooks.json"),
        format!(r#"{{"ho\u006fks":{{"PreToolUse":[{{"hooks":[{handler},false]}}]}}}}"#),
    )
    .unwrap();
    let result = scan(&[source(&root)], &[], "now");
    let rows: Vec<_> = result
        .items
        .iter()
        .filter(|i| i.kind == Kind::Hook)
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(result.issues.iter().any(|i| i.code == "hookConfigInvalid"));
    for row in rows {
        let text = if row.path.ends_with("hooks.json") {
            handler
        } else {
            inline
        };
        assert_eq!(row.bytes, Some(text.len() as u64));
        assert_eq!(
            row.estimate.as_ref().unwrap().content_hash,
            crate::hash(text)
        );
    }
}
#[test]
fn nested_hook_declarations_are_unknown_instead_of_mismeasured_or_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    fs::write(root.join("config.toml"), "[[hooks.PreToolUse]]\n[[hooks.PreToolUse.hooks]]\ntype='mcp_tool'\nserver='synthetic'\ntool='check'\n[hooks.PreToolUse.hooks.input]\nquery='example'\n").unwrap();
    let result = scan(&[source(&root)], &[], "now");
    let row = result.items.iter().find(|i| i.kind == Kind::Hook).unwrap();
    assert!(row.bytes.is_none() && row.content_tokens.is_none());
    assert_eq!(row.measurement_status, "declarationUnavailable");
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.code == "hookDeclarationMeasurementUnavailable")
    );
    assert!(!result.issues.iter().any(|i| i.code == "hookConfigInvalid"));
}
