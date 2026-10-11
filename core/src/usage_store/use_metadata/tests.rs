use super::super::timing_evidence::{TimingReadBudget, TurnTarget};
use super::*;
use std::sync::atomic::AtomicBool;
fn operation(id: &str, kind: &str, turn: Option<&str>) -> Arc<Operation> {
    Arc::new(serde_json::from_value(serde_json::json!({"id":id,"threadId":"thread","turnId":turn,"callId":id,"kind":kind,"name":"read_file","path":"/synthetic/skill/SKILL.md","server":"server","tool":"search","sequence":1,"timePrecision":"unknown","status":"failed","outcomeConflict":false,"evidence":[]})).unwrap())
}
fn data(operations: Vec<Arc<Operation>>) -> Collected {
    Collected {
        operations,
        threads: vec![Thread {
            id: "thread".into(),
            source_instance_id: "source".into(),
            agent_kind: "codex".into(),
            upstream_id: "native".into(),
            title: None,
            project: None,
            started_at: None,
            last_activity_at: None,
        }],
        turns: vec![Turn {
            id: "turn".into(),
            thread_id: "thread".into(),
            upstream_id: "turn-native".into(),
            ordinal: 1,
            started_at: None,
            ended_at: None,
            last_activity_at: None,
            status: "completed".into(),
        }],
        ..Default::default()
    }
}
fn target() -> TurnTarget<'static> {
    TurnTarget {
        source: "source",
        thread: "thread",
        turn: "turn",
    }
}
fn code(error: anyhow::Error) -> &'static str {
    error
        .downcast_ref::<crate::dto::OperationError>()
        .unwrap()
        .code
}
#[test]
fn read_modes_metadata_count_only_related_unassigned_use_candidates() {
    let root = tempfile::tempdir().unwrap();
    let mut wrapper = operation("wrapper", "skillRead", None).as_ref().clone();
    wrapper.name = "read_skill_file".into();
    let mut generic = operation("generic", "tool", None).as_ref().clone();
    generic.path = None;
    let mut foreign = operation("foreign", "skillRead", None).as_ref().clone();
    foreign.thread_id = "foreign-thread".into();
    let mut collected = data(vec![
        operation("assigned", "skillRead", Some("turn")),
        operation("skill", "skillRead", None),
        Arc::new(wrapper),
        Arc::new(generic),
        operation("catalog", "skillCatalog", None),
        operation("discovery", "mcpDiscovery", None),
        operation("mcp", "mcpResource", None),
        operation("ambiguity", "mcpUnclassified", None),
        operation("empty-turn", "mcpTool", Some("")),
        Arc::new(foreign),
    ]);
    let mut thread = collected.threads[0].clone();
    thread.id = "foreign-thread".into();
    thread.source_instance_id = "foreign-source".into();
    collected.threads.push(thread);
    let disk = super::super::save_at(root.path(), collected.clone()).unwrap();
    let memory = super::super::memory(
        collected,
        "live:uses".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    for snapshot in [&memory, &disk] {
        let evidence = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(
            evidence.unassigned_uses.method_version,
            usage_observations::METHOD_VERSION
        );
        assert_eq!(evidence.unassigned_uses.skill_records, 2);
        assert_eq!(evidence.unassigned_uses.mcp_records, 3);
        assert_eq!(evidence.operations.len(), 1);
        let foreign = snapshot
            .manifest
            .threads
            .iter()
            .find(|entry| entry.thread.id == "foreign-thread")
            .unwrap();
        assert_eq!(foreign.unassigned_uses.skill_records, 1);
        assert_eq!(foreign.unassigned_uses.mcp_records, 0);
    }
}
#[test]
fn unrelated_unassigned_operation_slice_is_not_read_for_membership_proof() {
    use std::io::{Seek, SeekFrom, Write};
    let root = tempfile::tempdir().unwrap();
    let collected = data(vec![
        operation("assigned", "skillRead", Some("turn")),
        operation("unassigned", "skillRead", None),
    ]);
    let disk = super::super::save_at(root.path(), collected).unwrap();
    let owner = &disk.manifest.threads[0];
    let unassigned = &owner.turns["unassigned"];
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(disk.directory.join(&owner.file.file))
        .unwrap();
    file.seek(SeekFrom::Start(unassigned.slice.offset)).unwrap();
    file.write_all(b"!").unwrap();
    let out = disk
        .timing_evidence(
            target(),
            TimingReadBudget::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(out.operations.len(), 1);
    assert_eq!(out.unassigned_uses.skill_records, 1);
}
#[test]
fn missing_current_metadata_is_corrupt_and_future_headers_precede_new_shapes_without_rewrite() {
    let root = tempfile::tempdir().unwrap();
    let disk = super::super::save_at(root.path(), data(vec![])).unwrap();
    let path = disk.directory.join("manifest.json");
    let original = fs::read(&path).unwrap();
    for (metadata, expected) in [
        (None, "SNAPSHOT_CORRUPT"),
        (
            Some(
                serde_json::json!({"methodVersion":crate::usage_observations::METHOD_VERSION - 1,"skillRecords":0,"mcpRecords":0}),
            ),
            "UNSUPPORTED_VERSION",
        ),
        (
            Some(
                serde_json::json!({"methodVersion":crate::usage_observations::METHOD_VERSION,"skillRecords":null,"mcpRecords":0}),
            ),
            "SNAPSHOT_CORRUPT",
        ),
        (
            Some(
                serde_json::json!({"methodVersion":crate::usage_observations::METHOD_VERSION + 1,"futureFields":{"not":"current"}}),
            ),
            "UNSUPPORTED_VERSION",
        ),
    ] {
        let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let thread = manifest["threads"][0].as_object_mut().unwrap();
        if let Some(metadata) = metadata {
            thread.insert("unassignedUses".into(), metadata);
        } else {
            thread.remove("unassignedUses");
        }
        let bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            code(
                super::super::load_at(root.path(), Some(&disk.manifest.snapshot_ref.snapshot_id))
                    .err()
                    .unwrap()
            ),
            expected
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(disk.directory.join("ledger.json").exists());
    }
    fs::write(&path, original).unwrap();
    assert!(
        super::super::load_at(root.path(), Some(&disk.manifest.snapshot_ref.snapshot_id)).is_ok()
    );
}
#[test]
fn typed_memory_metadata_unknown_version_does_not_become_zero() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot = super::super::memory(
        data(vec![]),
        "live:uses".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    Arc::make_mut(&mut snapshot.manifest.threads[0])
        .unassigned_uses
        .method_version = crate::usage_observations::METHOD_VERSION + 1;
    assert_eq!(
        code(
            snapshot
                .timing_evidence(
                    target(),
                    TimingReadBudget::default(),
                    &AtomicBool::new(false)
                )
                .unwrap_err()
        ),
        "UNSUPPORTED_VERSION"
    );
}

#[test]
fn read_modes_native_multi_target_candidates_charge_one_unassigned_operation_and_exclude_user_shell()
 {
    let root = tempfile::tempdir().unwrap();
    let mut native = operation("native", "command", None).as_ref().clone();
    native.path = None;
    native.work=Some(serde_json::from_value(serde_json::json!({"formatVersion":crate::adapters::contract::WORK_OBSERVATION_VERSION,"stage":"terminal","data":{"kind":"command","cwd":"/synthetic","source":"agent","parsed_commands":[{"kind":"read","path":"a/SKILL.md"},{"kind":"read","path":"b/SKILL.md"},{"kind":"read","path":"a/SKILL.md"}]},"gaps":[]})).unwrap());
    let mut user = native.clone();
    user.id = "user".into();
    let crate::adapters::contract::WorkData::Command { source, .. } =
        &mut user.work.as_mut().unwrap().data
    else {
        panic!()
    };
    *source = Some(crate::adapters::contract::CommandSource::UserShell);
    let collected = data(vec![
        operation("target", "skillRead", Some("turn")),
        Arc::new(native),
        Arc::new(user),
    ]);
    let disk = super::super::save_at(root.path(), collected.clone()).unwrap();
    let memory = super::super::memory(
        collected,
        "live:multi".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    for snapshot in [&memory, &disk] {
        let evidence = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(
            evidence.unassigned_uses.method_version,
            crate::usage_observations::METHOD_VERSION
        );
        assert_eq!(evidence.unassigned_uses.skill_records, 1);
    }
}
