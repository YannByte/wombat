use super::*;

fn start(call: &str) -> Value {
    json!({"type":"response_item","payload":{"type":"function_call","call_id":call,"name":"read_file","arguments":"{\"path\":\"/synthetic/AGENTS.md\"}"}})
}
fn output(call: &str, output: Value) -> Value {
    json!({"type":"response_item","payload":{"type":"function_call_output","call_id":call,"output":output}})
}
#[test]
fn read_result_hashes_bind_native_bytes_without_retaining_bodies_and_conflicts_are_sticky() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "sessions/read.jsonl",
        &[
            meta("t"),
            start("call"),
            output("call", json!("PRIVATE_READ_BODY\n")),
        ],
    );
    let result = collect(dir.path());
    let observed = result.operations[0].text_result.as_ref().unwrap();
    assert_eq!(
        observed.hash.as_deref(),
        Some(crate::hash("PRIVATE_READ_BODY\n").as_str())
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("PRIVATE_READ_BODY")
    );
    write(
        dir.path(),
        "sessions/read.jsonl",
        &[
            meta("t"),
            start("call"),
            output("call", json!("PRIVATE_READ_BODY\n")),
            output("call", json!("DIFFERENT_BODY")),
            output("call", json!("PRIVATE_READ_BODY\n")),
        ],
    );
    let result = collect(dir.path());
    let observed = result.operations[0].text_result.as_ref().unwrap();
    assert!(observed.conflicting);
    assert_eq!(observed.hash, None);
}

#[test]
fn native_agents_metadata_records_load_without_retaining_or_trusting_user_text() {
    let dir = tempfile::tempdir().unwrap();
    let project = test_absolute("synthetic/project");
    write(
        dir.path(),
        "sessions/instructions.jsonl",
        &[
            meta("t"),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:00Z","payload":{"type":"message","id":"native","role":"user","content":[{"type":"input_text","text":format!("# AGENTS.md instructions for {project}\n\n<INSTRUCTIONS>PRIVATE_RULE_BODY</INSTRUCTIONS>")}],"internal_chat_message_metadata_passthrough":{"turn_id":"u","content_item_kinds":["agents_md.instructions"]}}}),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:01Z","payload":{"type":"message","id":"ordinary","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /spoofed"}],"internal_chat_message_metadata_passthrough":{"turn_id":"u","content_item_kinds":["user_prompt"]}}}),
        ],
    );
    let result = collect(dir.path());
    assert_eq!(result.operations.len(), 1);
    let operation = &result.operations[0];
    assert_eq!(operation.kind.as_ref(), "instructionLoad");
    assert_eq!(operation.status.as_ref(), "completed");
    let expected = Path::new(&project)
        .join("AGENTS.md")
        .to_string_lossy()
        .into_owned();
    assert_eq!(operation.path.as_deref(), Some(expected.as_str()));
    assert!(operation.turn_id.is_some());
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("PRIVATE_RULE_BODY")
    );
    assert!(!serde_json::to_string(&result).unwrap().contains("spoofed"));
}

#[test]
fn native_instruction_identity_deduplicates_replays_without_merging_document_targets() {
    let dir = tempfile::tempdir().unwrap();
    let first = test_absolute("synthetic/first");
    let second = test_absolute("synthetic/second");
    let message = |body: &str| {
        json!({"type":"response_item","timestamp":"2026-09-29T00:00:00Z","payload":{
            "type":"message","id":"native","role":"user","content":[
                {"type":"input_text","text":format!("# AGENTS.md instructions for {first}\n\n<INSTRUCTIONS>{body}</INSTRUCTIONS>")},
                {"type":"input_text","text":format!("# AGENTS.md instructions for {second}\n\n<INSTRUCTIONS>SECOND_PRIVATE_BODY</INSTRUCTIONS>")}
            ],"internal_chat_message_metadata_passthrough":{"turn_id":"u","content_item_kinds":["agents_md.instructions","agents_md.instructions"]}
        }})
    };
    write(
        dir.path(),
        "sessions/instructions.jsonl",
        &[
            meta("t"),
            message("FIRST_PRIVATE_BODY"),
            message("FIRST_PRIVATE_BODY"),
            message("CONFLICTING_PRIVATE_BODY"),
        ],
    );
    let result = collect(dir.path());
    assert_eq!(result.operations.len(), 2);
    for operation in &result.operations {
        assert!(operation.item_id.is_some());
        assert!(operation.call_id.is_none());
        let expected_conflict = operation.path.as_deref()
            == Some(
                Path::new(&first)
                    .join("AGENTS.md")
                    .to_string_lossy()
                    .as_ref(),
            );
        assert_eq!(
            operation.text_result.as_ref().unwrap().conflicting,
            expected_conflict
        );
    }
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("PRIVATE_BODY")
    );
}

#[test]
fn native_skill_catalog_and_observed_use_keep_only_resolved_identity() {
    let dir = tempfile::tempdir().unwrap();
    let root = test_absolute("synthetic/project/.agents/skills");
    let skill = Path::new(&root)
        .join("review")
        .join("SKILL.md")
        .to_string_lossy()
        .into_owned();
    let catalog = format!(
        "<skills_instructions>\n### Skill roots\n- `r0` = `{root}`\n### Available skills\n- review: PRIVATE_DESCRIPTION (file: r0/review/SKILL.md)\n</skills_instructions>"
    );
    let command = format!("cat '{skill}'");
    let exec = format!(
        "text(await tools.exec_command({{cmd:{},max_output_tokens:1000}}));",
        serde_json::to_string(&command).unwrap()
    );
    write(
        dir.path(),
        "sessions/skills.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            json!({"type":"event_msg","timestamp":"2026-09-29T00:00:00Z","payload":{"type":"task_started","turn_id":"u"}}),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:01Z","payload":{"type":"message","id":"catalog","role":"developer","content":[{"type":"input_text","text":catalog}],"internal_chat_message_metadata_passthrough":{"turn_id":"u","content_item_kinds":["host_skills.instructions"]}}}),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:02Z","payload":{"type":"message","id":"declaration","role":"assistant","content":[{"type":"output_text","text":"我会使用 review Skill。"}],"internal_chat_message_metadata_passthrough":{"turn_id":"u","content_item_kinds":["unknown"]}}}),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:03Z","payload":{"type":"custom_tool_call","call_id":"exec","name":"exec","status":"completed","input":exec,"internal_chat_message_metadata_passthrough":{"turn_id":"u"}}}),
        ],
    );
    let result = collect(dir.path());
    assert_eq!(
        result
            .operations
            .iter()
            .filter(|operation| operation.kind.as_ref() == "skillAvailable")
            .count(),
        1
    );
    for kind in ["skillAvailable", "skillUse", "skillRead"] {
        let operation = result
            .operations
            .iter()
            .find(|operation| operation.kind.as_ref() == kind)
            .unwrap_or_else(|| {
                panic!(
                    "missing {kind}; found {:?}",
                    result
                        .operations
                        .iter()
                        .map(|operation| operation.kind.as_ref())
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(operation.path.as_deref(), Some(skill.as_str()));
        assert_eq!(
            operation.turn_id.as_deref(),
            Some(result.turns[0].id.as_str())
        );
    }
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("PRIVATE_DESCRIPTION"));
    assert!(!serialized.contains("我会使用"));
}

#[test]
fn unknown_outputs_end_running_without_claiming_success_or_reopening_on_replay() {
    let dir = tempfile::tempdir().unwrap();
    let mut rows = vec![meta("t"), context("u", "gpt-5.4", "high")];
    for (call, response_first, result, expected) in [
        ("forward", false, json!("PRIVATE_UNKNOWN_RESULT"), "unknown"),
        ("reverse", true, json!("PRIVATE_UNKNOWN_RESULT"), "unknown"),
        ("success", false, json!({"isError":false}), "completed"),
        ("failure", false, json!({"isError":true}), "failed"),
    ] {
        let begin = start(call);
        let end = output(call, result);
        rows.extend(if response_first {
            [end.clone(), begin.clone()]
        } else {
            [begin.clone(), end.clone()]
        });
        rows.extend([begin, output(call, json!("PRIVATE_UNKNOWN_RESULT")), end]);
        let path = write(dir.path(), "sessions/a.jsonl", &rows);
        let result = collect(dir.path());
        let op = result
            .operations
            .iter()
            .find(|o| o.call_id.as_deref() == Some(call))
            .unwrap();
        assert_eq!(op.status.as_ref(), expected);
        assert_eq!(op.name.as_ref(), "read_file");
        assert_eq!(op.path.as_deref(), Some("/synthetic/AGENTS.md"));
        assert_eq!(op.evidence.len(), 5);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("PRIVATE_UNKNOWN_RESULT")
        );
        assert!(path.exists());
    }
}

#[test]
fn unknown_native_mcp_completion_and_forked_output_replace_running() {
    let dir = tempfile::tempdir().unwrap();
    let invocation =
        json!({"server":"docs","tool":"search","arguments":{"private":"PRIVATE_ARGUMENT"}});
    write(
        dir.path(),
        "sessions/parent.jsonl",
        &[
            meta("parent"),
            context("u", "gpt-5.4", "high"),
            start("file"),
            json!({"type":"event_msg","payload":{"type":"mcp_tool_call_begin","call_id":"mcp","turn_id":"u","invocation":invocation}}),
            json!({"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"mcp","turn_id":"u","invocation":invocation,"result":null}}),
        ],
    );
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        dir.path(),
        "sessions/child.jsonl",
        &[
            child,
            context("u", "gpt-5.4", "high"),
            start("file"),
            output("file", json!("PRIVATE_UNKNOWN_RESULT")),
        ],
    );
    let result = collect(dir.path());
    assert_eq!(result.operations.len(), 2);
    assert!(
        result
            .operations
            .iter()
            .all(|o| o.status.as_ref() == "unknown")
    );
    let parent = result
        .threads
        .iter()
        .find(|t| t.upstream_id == "parent")
        .unwrap();
    assert!(
        result
            .operations
            .iter()
            .all(|o| o.thread_id.as_ref() == parent.id)
    );
    assert!(!serde_json::to_string(&result).unwrap().contains("PRIVATE_"));
}

#[test]
fn unknown_output_survives_incremental_sync_restart_and_retains_the_previous_view() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/live.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 20, 10),
            start("file"),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = incremental::Cache::default();
    let first = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(first.collected.operations[0].status.as_ref(), "running");
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        output("file", json!("PRIVATE_UNKNOWN_RESULT"))
    )
    .unwrap();
    let second = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(second.collected.operations[0].status.as_ref(), "unknown");
    assert_eq!(first.collected.operations[0].status.as_ref(), "running");
    // An append forces the restarted parser to load its persisted facts.
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{}",
        start("file")
    )
    .unwrap();
    let restarted =
        incremental::sync_cached(&db, &source, false, &mut incremental::Cache::default())
            .unwrap()
            .unwrap()
            .collected;
    assert_eq!(restarted.operations[0].status.as_ref(), "unknown");
    assert_eq!(second.collected.operations[0].status.as_ref(), "unknown");
    assert_eq!(restarted.operations, collect(root.path()).operations);
    assert_eq!(restarted.measurements[0].tokens.total, Some(110));
    assert!(
        !serde_json::to_string(&restarted)
            .unwrap()
            .contains("PRIVATE_UNKNOWN_RESULT")
    );
}

mod outcomes;
