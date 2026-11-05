use super::*;

#[test]
fn declined_native_read_retains_dispatch_uncertainty_after_exit_code_reconciliation() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/declined.jsonl",
        &[
            meta("t"),
            json!({"type":"event_msg","timestamp":"2026-10-04T00:00:00Z","payload":{"type":"item_completed","thread_id":"t","turn_id":"u","item":{"type":"CommandExecution","id":"declined","cwd":"/synthetic","source":"unified_exec_startup","command":["/bin/sh","-lc","cat /synthetic/SKILL.md"],"parsed_cmd":[{"type":"read","path":"/synthetic/SKILL.md"}],"status":"declined","exit_code":1}}}),
        ],
    );
    let out = collect(root.path());
    let op = &out.operations[0];
    assert_eq!(op.status.as_ref(), "failed");
    assert!(
        op.matching
            .as_ref()
            .unwrap()
            .gaps
            .contains(&MatchGap::UnconfirmedRead)
    );
    assert!(
        crate::usage_observations::read_targets(op)
            .unwrap()
            .candidate
    );
    assert_eq!(crate::usage_observations::use_kind(op), None);
}
fn cmd(id: &str, phase: &str, parsed: Value, source: Value) -> Value {
    json!({"type":"event_msg","timestamp":"2026-10-05T01:00:00Z","payload":{"type":phase,"turn_id":"turn","item":{"type":"CommandExecution","id":id,"cwd":"file:///synthetic/command-cwd","parsed_cmd":parsed,"source":source,"command":["PRIVATE_COMMAND","PRIVATE_ARG"],"aggregated_output":"PRIVATE_OUTPUT","interaction_input":"PRIVATE_INTERACTION","status":if phase == "item_started" {"in_progress"} else {"completed"}}}})
}
fn parsed() -> Value {
    json!([
        {"type":"read","path":"skills/one/SKILL.md","cmd":"PRIVATE_READ","name":"PRIVATE_NAME"},
        {"type":"read","path":"skills/two/SKILL.md","cmd":"PRIVATE_READ"},
        {"type":"read","path":"skills/one/SKILL.md"},
        {"type":"list_files","path":"src","cmd":"PRIVATE_LIST"},
        {"type":"search","path":"src","query":"PRIVATE_QUERY","cmd":"PRIVATE_SEARCH"},
        {"type":"unknown","cmd":"PRIVATE_SCRIPT"}
    ])
}
fn work(op: &Operation) -> &WorkObservation {
    op.work.as_ref().unwrap()
}
fn commands(op: &Operation) -> Option<&Vec<ParsedCommand>> {
    match &work(op).data {
        WorkData::Command {
            parsed_commands, ..
        } => parsed_commands.as_ref(),
        _ => panic!("command expected"),
    }
}

#[test]
fn native_multi_read_preserves_source_order_on_one_operation_and_never_retains_text() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/cmd.jsonl",
        &[
            meta("t"),
            cmd("c", "item_started", parsed(), json!("agent")),
            cmd("c", "item_completed", parsed(), json!("agent")),
        ],
    );
    let out = collect(root.path());
    assert_eq!(out.operations.len(), 1);
    let op = &out.operations[0];
    assert_eq!(op.kind.as_ref(), "command");
    assert!(
        op.path.is_none(),
        "multi-target metadata does not choose one path"
    );
    assert_eq!(commands(op).unwrap().len(), 6);
    assert_eq!(commands(op).unwrap()[0], commands(op).unwrap()[2]);
    assert!(
        matches!(&commands(op).unwrap()[1],ParsedCommand::Read{path:Some(p)} if p=="skills/two/SKILL.md")
    );
    assert!(
        matches!(&work(op).data,WorkData::Command{cwd:Some(cwd),source:Some(CommandSource::Agent),..} if cwd=="file:///synthetic/command-cwd")
    );
    assert_eq!(work(op).stage, WorkStage::Terminal);
    assert!(work(op).gaps.is_empty());
    assert!(
        out.threads[0]
            .project
            .as_deref()
            .is_none_or(|p| !p.contains("command-cwd"))
    );
    assert!(!serde_json::to_string(&out).unwrap().contains("PRIVATE_"));
}

#[test]
fn source_variants_are_closed_and_missing_fields_do_not_default_to_agent_or_zero() {
    let root = tempfile::tempdir().unwrap();
    let mut missing = cmd("missing", "item_completed", Value::Null, Value::Null);
    for key in ["cwd", "parsed_cmd", "source"] {
        missing["payload"]["item"]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    let variants = [
        "agent",
        "user_shell",
        "unified_exec_startup",
        "unified_exec_interaction",
        "future_source",
    ];
    let mut rows = vec![meta("t"), missing];
    rows.extend(
        variants
            .iter()
            .map(|s| cmd(s, "item_completed", json!([]), json!(s))),
    );
    write(root.path(), "sessions/sources.jsonl", &rows);
    let out = collect(root.path());
    let missing = out
        .operations
        .iter()
        .find(|o| o.item_id.as_deref() == Some("missing"))
        .unwrap();
    assert!(commands(missing).is_none());
    assert!(work(missing).gaps.contains(&WorkGap::MissingCommandSource));
    assert!(work(missing).gaps.contains(&WorkGap::MissingCommandCwd));
    assert!(work(missing).gaps.contains(&WorkGap::MissingParsedCommands));
    for op in out
        .operations
        .iter()
        .filter(|o| o.item_id.as_deref() != Some("missing"))
    {
        assert_eq!(commands(op).unwrap().len(), 0);
    }
    let future = out
        .operations
        .iter()
        .find(|o| o.item_id.as_deref() == Some("future_source"))
        .unwrap();
    assert!(matches!(
        &work(future).data,
        WorkData::Command {
            source: Some(CommandSource::Unknown),
            ..
        }
    ));
    assert!(work(future).gaps.contains(&WorkGap::UnknownVariant));
}

#[test]
fn invalid_and_future_entries_keep_explicit_gaps_without_decoding_unknown_paths() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/unknown.jsonl",
        &[
            meta("t"),
            cmd(
                "unknown",
                "item_completed",
                json!([
                    {"type":"read"},{"type":"read","path":12},{"type":"future","path":"PRIVATE_UNKNOWN_PATH","cmd":"PRIVATE_UNKNOWN_CMD"},{"type":"list_files"},{"type":"search"},{"type":"unknown","path":"PRIVATE_UNKNOWN_PATH"}
                ]),
                json!("agent"),
            ),
            cmd(
                "bad",
                "item_completed",
                json!({"cmd":"PRIVATE_INVALID"}),
                json!(false),
            ),
        ],
    );
    let out = collect(root.path());
    let op = out
        .operations
        .iter()
        .find(|o| o.item_id.as_deref() == Some("unknown"))
        .unwrap();
    assert_eq!(commands(op).unwrap().len(), 6);
    assert_eq!(commands(op).unwrap()[0], ParsedCommand::Read { path: None });
    for gap in [
        WorkGap::MissingReadPath,
        WorkGap::InvalidField,
        WorkGap::UnknownVariant,
    ] {
        assert!(work(op).gaps.contains(&gap));
    }
    assert!(
        commands(
            out.operations
                .iter()
                .find(|o| o.item_id.as_deref() == Some("bad"))
                .unwrap()
        )
        .is_none()
    );
    assert!(!serde_json::to_string(&out).unwrap().contains("PRIVATE_"));
}

#[test]
fn endpoint_missing_metadata_fills_from_dispatch_but_conflicts_lock_unknown() {
    let root = tempfile::tempdir().unwrap();
    let mut absent = cmd("fill", "item_completed", Value::Null, Value::Null);
    absent["payload"]["item"]
        .as_object_mut()
        .unwrap()
        .remove("cwd");
    let mut other = cmd("conflict", "item_completed", parsed(), json!("user_shell"));
    other["payload"]["item"]["cwd"] = json!("file:///different");
    write(
        root.path(),
        "sessions/endpoints.jsonl",
        &[
            meta("t"),
            cmd("fill", "item_started", parsed(), json!("agent")),
            absent,
            cmd("conflict", "item_started", parsed(), json!("agent")),
            other,
            cmd("conflict", "item_completed", parsed(), json!("agent")),
        ],
    );
    let out = collect(root.path());
    let find = |id| {
        out.operations
            .iter()
            .find(|op| op.item_id.as_deref() == Some(id))
            .unwrap()
    };
    assert_eq!(commands(find("fill")).unwrap().len(), 6);
    assert!(work(find("fill")).gaps.is_empty());
    assert_eq!(work(find("fill")).stage, WorkStage::Terminal);
    assert!(commands(find("conflict")).is_none());
    assert_eq!(work(find("conflict")).stage, WorkStage::Terminal);
    assert!(
        work(find("conflict"))
            .gaps
            .contains(&WorkGap::ConflictingObservation)
    );
}

#[test]
fn metadata_budget_omits_whole_list_and_transient_or_wrapper_text_is_not_native() {
    let root = tempfile::tempdir().unwrap();
    let huge = (0..=WORK_PATH_LIMIT)
        .map(|_| json!({"type":"unknown","cmd":"PRIVATE_LARGE"}))
        .collect::<Vec<_>>();
    write(
        root.path(),
        "sessions/budget.jsonl",
        &[
            meta("t"),
            cmd("huge", "item_completed", json!(huge), json!("agent")),
            cmd(
                "huge-path",
                "item_completed",
                json!([{"type":"read","path":"x".repeat(WORK_PATH_BYTES+1)}]),
                json!("agent"),
            ),
            json!({"type":"event_msg","payload":{"type":"exec_command_end","call_id":"transient","turn_id":"turn","parsed_cmd":parsed(),"cwd":"file:///synthetic","source":"agent"}}),
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"wrapper","name":"exec_command","arguments":"PRIVATE_WRAPPER","parsed_cmd":parsed(),"source":"agent"}}),
        ],
    );
    let out = collect(root.path());
    assert_eq!(out.operations.len(), 3);
    for op in out
        .operations
        .iter()
        .filter(|o| o.kind.as_ref() == "command")
    {
        assert!(commands(op).is_none());
        assert!(work(op).gaps.contains(&WorkGap::ResourceLimit));
    }
    assert!(
        out.operations
            .iter()
            .find(|o| o.call_id.as_deref() == Some("wrapper"))
            .unwrap()
            .work
            .is_none()
    );
    assert!(!serde_json::to_string(&out).unwrap().contains("PRIVATE_"));
}

#[test]
fn command_metadata_survives_incremental_restart_and_explicit_fork_replay() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("parent"),
            cmd("c", "item_completed", parsed(), json!("agent")),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let run = || {
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let tx = db.transaction().unwrap();
        let out = incremental::sync(&tx, &source, false).unwrap();
        tx.commit().unwrap();
        out
    };
    let first = run().unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(
            (cmd("c", "item_completed", parsed(), json!("agent")).to_string() + "\n").as_bytes(),
        )
        .unwrap();
    let second = run().unwrap();
    assert_eq!(second.operations.len(), 1);
    assert_eq!(
        commands(&first.operations[0]),
        commands(&second.operations[0])
    );
    let mut fork = meta("child");
    fork["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[fork, cmd("c", "item_completed", parsed(), json!("agent"))],
    );
    let forked = run().unwrap();
    assert_eq!(forked.operations.len(), 1);
    assert_eq!(commands(&forked.operations[0]).unwrap().len(), 6);
    assert_eq!(
        serde_json::to_value(&forked.operations).unwrap(),
        serde_json::to_value(&collect(root.path()).operations).unwrap()
    );
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let saved: String = db
        .query_row("SELECT group_concat(json(payload)) FROM entries", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!saved.contains("PRIVATE_"));
}

fn matching(op: &Operation) -> &OperationMatchObservation {
    op.matching.as_ref().expect("matching observation expected")
}
#[test]
fn native_command_matching_is_retained_in_safe_events_and_excludes_inherited_receivers() {
    let root = tempfile::tempdir().unwrap();
    let mut start = cmd(
        "read",
        "item_started",
        json!([{ "type":"read","path":"a"}]),
        json!("agent"),
    );
    start["payload"]["item"]
        .as_object_mut()
        .unwrap()
        .remove("interaction_input");
    start["payload"]["item"]["command"] = json!(["/bin/zsh", "-lc", "cat a"]);
    let mut end = start.clone();
    end["payload"]["type"] = json!("item_completed");
    end["payload"]["item"]["status"] = json!("completed");
    let mut inherited = start.clone();
    inherited["payload"]["item"]["id"] = json!("inherited");
    inherited["metadata"] = json!({"inherited_user_message":true});
    write(
        root.path(),
        "sessions/matching.jsonl",
        &[
            meta("t"),
            context("turn", "model", "high"),
            start,
            end,
            inherited,
        ],
    );
    let out = collect(root.path());
    let op = out
        .operations
        .iter()
        .find(|op| op.item_id.as_deref() == Some("read"))
        .unwrap();
    let copy = out
        .events
        .iter()
        .find_map(|event| match event.payload() {
            crate::session_events::Payload::Operation { value, .. }
                if value.item_id.as_deref() == Some("inherited") =>
            {
                assert!(event.inherited());
                Some(value.as_ref())
            }
            _ => None,
        })
        .unwrap();
    assert!(
        !out.operations
            .iter()
            .any(|operation| operation.item_id.as_deref() == Some("inherited"))
    );
    assert!(matching(copy).receiver_owner.is_none());
    assert!(matching(op).request_fingerprint.is_some());
    assert_eq!(
        matching(op).receiver_owner.as_deref(),
        Some(op.thread_id.as_ref())
    );
    assert_eq!(
        matching(op).read_targets[0].path,
        "/synthetic/command-cwd/a"
    );
    let mut observed = 0;
    for event in &out.events {
        if let crate::session_events::Payload::Operation { value, .. } = event.payload() {
            assert_eq!(
                matching(value).receiver_owner.as_deref(),
                if value.item_id.as_deref() == Some("inherited") {
                    None
                } else {
                    Some(value.thread_id.as_ref())
                }
            );
            assert!(matching(value).request_fingerprint.is_some());
            observed += 1;
            let encoded = serde_json::to_string(event).unwrap();
            assert!(!encoded.contains("cat a"));
            assert!(!encoded.contains("PRIVATE_OUTPUT"));
            let restored: crate::session_events::Event = serde_json::from_str(&encoded).unwrap();
            assert_eq!(restored.id(), event.id());
            let mut forged = serde_json::to_value(&restored).unwrap();
            forged["payload"]["value"]["work"]["data"]["matching"]["receiverOwner"] =
                json!("foreign-owner");
            assert!(serde_json::from_value::<crate::session_events::Event>(forged).is_err());
            if value.item_id.as_deref() != Some("inherited") {
                let mut forged = serde_json::to_value(&restored).unwrap();
                forged["payload"]["value"]["work"]["data"]["source"] = json!("user_shell");
                assert!(serde_json::from_value::<crate::session_events::Event>(forged).is_err());
            }
        }
    }
    assert_eq!(observed, 3);
}
#[test]
fn command_parameter_disagreement_clears_matching_without_erasing_independent_work_paths() {
    let root = tempfile::tempdir().unwrap();
    let mut a = cmd(
        "same",
        "item_started",
        json!([{ "type":"read","path":"a"}]),
        json!("agent"),
    );
    a["payload"]["item"]
        .as_object_mut()
        .unwrap()
        .remove("interaction_input");
    a["payload"]["item"]["command"] = json!(["cat", "a"]);
    let mut b = a.clone();
    b["payload"]["type"] = json!("item_completed");
    b["payload"]["item"]["command"] = json!(["cat", "-n", "a"]);
    write(root.path(), "sessions/conflict.jsonl", &[meta("t"), a, b]);
    let out = collect(root.path());
    let value = matching(&out.operations[0]);
    assert!(value.request_fingerprint.is_none());
    assert!(value.gaps.contains(&MatchGap::ConflictingObservation));
    assert!(commands(&out.operations[0]).is_some());
}
