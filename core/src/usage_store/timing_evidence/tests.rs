//! Independent canonical turn and physical block fixtures.
use super::*;
use crate::session_events::{Gap, LifecycleKind, Payload, Phase, Time};
fn fact() -> Arc<Measurement> {
    Arc::new(serde_json::from_value(serde_json::json!({ "id":"fact", "agentKind":"codex", "sourceInstanceId":"source", "threadId":"thread", "turnId":"turn", "grain":"response", "timePrecision":"unknown", "model":{}, "tokens":{}, "tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":"missing","rawInput":"missing"}, "pricingContextConflict":false,"requestScoped":true, "sequence":1, "evidence":[] })).unwrap())
}
fn event(offset: u64, thread: Option<&str>, turn: Option<&str>, gap: bool) -> Arc<Event> {
    Arc::new(
        Event::new(
            Position {
                source_instance_id: "source".into(),
                file_id: "file".into(),
                generation: "generation".into(),
                byte_offset: offset,
                ordinal: u32::from(turn.is_some()),
            },
            thread.map(str::to_owned),
            turn.map(str::to_owned),
            Time::from_source(None).0,
            if gap {
                vec![Gap::SourcePartial]
            } else {
                vec![]
            },
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Turn,
                phase: Phase::Completed,
                native_id: None,
                duration_ms: Some(0),
                first_token_ms: None,
            },
        )
        .unwrap(),
    )
}
fn facts(events: Vec<Arc<Event>>) -> Collected {
    Collected {
        threads: vec![Thread {
            id: "thread".into(),
            agent_kind: "codex".into(),
            source_instance_id: "source".into(),
            upstream_id: "native".into(),
            title: None,
            project: None,
            started_at: None,
            last_activity_at: None,
        }],
        measurements: vec![fact()],
        events,
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
fn code(error: anyhow::Error) -> String {
    error
        .downcast_ref::<crate::dto::OperationError>()
        .unwrap()
        .code
        .to_owned()
}
fn pair(root: &Path, facts: Collected) -> (Snapshot, Snapshot) {
    (
        memory(
            facts.clone(),
            "live:test".into(),
            crate::pricing_sync::current_at(root).unwrap(),
            None,
        )
        .unwrap(),
        save_at(root, facts).unwrap(),
    )
}
#[test]
fn timing_read_modes_share_all_charges() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![
        event(10, Some("thread"), Some("turn"), false),
        event(20, Some("thread"), Some("turn"), false),
        event(15, None, None, true),
        event(16, Some("thread"), None, true),
        event(17, None, None, false),
        event(30, None, None, true),
    ]);
    data.watermarks.push(SourceWatermark {
        original_source: None,
        format_version: WATERMARK_FORMAT_VERSION,
        source_instance_id: "source".into(),
        file_id: "file".into(),
        generation: Some("generation".into()),
        committed_offset: 40,
        observed_bytes: Some(40),
        observed_at: "2026-10-01T00:00:00Z".into(),
        state: WatermarkState::Complete,
        issue_codes: vec![],
    });
    let (memory, disk) = pair(root.path(), data);
    let cancel = AtomicBool::new(false);
    let a = memory
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    let b = disk
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    assert_eq!(a.coverage, b.coverage);
    assert_eq!(a.measurements, b.measurements);
    assert_eq!(
        a.controls
            .iter()
            .map(|e| e.position().byte_offset)
            .collect::<Vec<_>>(),
        vec![15, 16]
    );
    assert_eq!(
        b.controls.iter().map(|e| e.id()).collect::<Vec<_>>(),
        a.controls.iter().map(|e| e.id()).collect::<Vec<_>>()
    );
    assert_eq!(a.snapshot_unassigned_total, 3);
    assert_eq!(a.thread_unassigned_total, 1);
    assert_eq!(a.domains.len(), 1);
    assert!(a.domains[0].generation_matches);
}
#[test]
fn hard_facts_bytes_and_metadata_limits_fail_in_both_modes() {
    let root = tempfile::tempdir().unwrap();
    let (memory, disk) = pair(
        root.path(),
        facts(vec![event(10, Some("thread"), Some("turn"), false)]),
    );
    let cancel = AtomicBool::new(false);
    let used = memory
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap()
        .coverage;
    for snapshot in [&memory, &disk] {
        for budget in [
            TimingReadBudget {
                max_facts: 1,
                ..Default::default()
            },
            TimingReadBudget {
                max_bytes: used.bytes - 1,
                ..Default::default()
            },
            TimingReadBudget {
                max_metadata: used.metadata - 1,
                ..Default::default()
            },
        ] {
            assert_eq!(
                code(
                    snapshot
                        .timing_evidence(target(), budget, &cancel)
                        .unwrap_err()
                ),
                "RESOURCE_LIMIT"
            );
        }
        let at = TimingReadBudget {
            max_facts: used.facts,
            max_bytes: used.bytes,
            max_metadata: used.metadata,
        };
        assert_eq!(
            snapshot
                .timing_evidence(target(), at, &cancel)
                .unwrap()
                .coverage,
            used
        );
    }
}

#[test]
fn oversized_unassigned_bucket_only_reads_intersecting_physical_blocks() {
    let root = tempfile::tempdir().unwrap();
    let mut events: Vec<_> = (0..600)
        .map(|offset| event(offset, None, None, true))
        .collect();
    events.extend([
        event(401, Some("thread"), Some("turn"), false),
        event(402, Some("thread"), Some("turn"), false),
    ]);
    let (memory, disk) = pair(root.path(), facts(events));
    let global = disk
        .event_partition(&EventTarget {
            thread_id: None,
            turn_id: None,
        })
        .unwrap()
        .unwrap();
    // Neither unrelated event blocks nor the ledger is needed by this reader.
    fs::remove_file(disk.directory.join(&global.chunks[0].file.file)).unwrap();
    fs::remove_file(disk.directory.join(&disk.manifest.ledger.file)).unwrap();
    let cancel = AtomicBool::new(false);
    let budget = TimingReadBudget {
        max_facts: 203,
        ..Default::default()
    };
    for snapshot in [&memory, &disk] {
        let out = snapshot.timing_evidence(target(), budget, &cancel).unwrap();
        assert_eq!(out.snapshot_unassigned_total, 600);
        assert_eq!(out.coverage.facts, 203);
        assert_eq!(out.coverage.event_blocks, 2);
        assert_eq!(
            out.controls
                .iter()
                .map(|e| e.position().byte_offset)
                .collect::<Vec<_>>(),
            vec![401, 402]
        );
    }
}

#[test]
fn source_scope_uses_thread_metadata_and_missing_targets_are_errors() {
    let root = tempfile::tempdir().unwrap();
    let (memory, disk) = pair(root.path(), facts(vec![]));
    let cancel = AtomicBool::new(false);
    for snapshot in [&memory, &disk] {
        for (target, expected) in [
            (
                TurnTarget {
                    source: "other",
                    ..target()
                },
                "INVALID_ARGUMENT",
            ),
            (
                TurnTarget {
                    thread: "native",
                    ..target()
                },
                "NOT_FOUND",
            ),
            (
                TurnTarget {
                    turn: "missing",
                    ..target()
                },
                "NOT_FOUND",
            ),
            (
                TurnTarget {
                    source: "",
                    ..target()
                },
                "INVALID_ARGUMENT",
            ),
        ] {
            assert_eq!(
                code(
                    snapshot
                        .timing_evidence(target, TimingReadBudget::default(), &cancel)
                        .unwrap_err()
                ),
                expected
            );
        }
    }
}

#[test]
fn missing_domains_and_watermarks_remain_unknown_not_complete() {
    let root = tempfile::tempdir().unwrap();
    let (memory, disk) = pair(
        root.path(),
        facts(vec![event(1, Some("thread"), Some("turn"), false)]),
    );
    let cancel = AtomicBool::new(false);
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(target(), TimingReadBudget::default(), &cancel)
            .unwrap();
        assert!(out.source.is_none());
        assert!(out.domains[0].watermark.is_none());
        assert!(!out.domains[0].generation_matches);
    }
    let (memory, disk) = pair(root.path(), facts(vec![]));
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(target(), TimingReadBudget::default(), &cancel)
            .unwrap();
        assert!(out.events.is_empty());
        assert!(out.domains.is_empty());
        assert_eq!(out.measurements.len(), 1);
    }
}

#[test]
fn cancellation_is_visible_before_work_and_at_every_meter_boundary() {
    let root = tempfile::tempdir().unwrap();
    let (memory, disk) = pair(root.path(), facts(vec![]));
    let cancel = AtomicBool::new(true);
    for snapshot in [&memory, &disk] {
        assert_eq!(
            code(
                snapshot
                    .timing_evidence(target(), TimingReadBudget::default(), &cancel)
                    .unwrap_err()
            ),
            "CANCELLED"
        );
    }
    cancel.store(false, Ordering::Relaxed);
    let mut meter = Meter::new(TimingReadBudget::default(), &cancel);
    meter.facts(1).unwrap();
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(code(meter.bytes(1).unwrap_err()), "CANCELLED");
    assert_eq!(code(meter.work(1).unwrap_err()), "CANCELLED");
}

#[test]
fn current_hash_corruption_and_unknown_event_version_propagate() {
    let root = tempfile::tempdir().unwrap();
    let (mut memory, mut disk) = pair(
        root.path(),
        facts(vec![event(1, Some("thread"), Some("turn"), false)]),
    );
    let cancel = AtomicBool::new(false);
    memory.manifest.events.version = 99;
    disk.manifest.events.version = 99;
    for snapshot in [&memory, &disk] {
        assert_eq!(
            code(
                snapshot
                    .timing_evidence(target(), TimingReadBudget::default(), &cancel)
                    .unwrap_err()
            ),
            "UNSUPPORTED_VERSION"
        );
    }
    disk.manifest.events.version = 2;
    let slice = &disk.manifest.threads[0].turns["turn"].slice;
    let path = disk.directory.join(&disk.manifest.threads[0].file.file);
    let mut bytes = fs::read(&path).unwrap();
    bytes[slice.offset as usize] ^= 1;
    fs::write(path, bytes).unwrap();
    assert_eq!(
        code(
            disk.timing_evidence(target(), TimingReadBudget::default(), &cancel)
                .unwrap_err()
        ),
        "SNAPSHOT_CORRUPT"
    );
}

fn replace_slice(snapshot: &mut Snapshot, bytes: &[u8]) {
    let thread = Arc::make_mut(&mut snapshot.manifest.threads[0]);
    fs::write(snapshot.directory.join(&thread.file.file), bytes).unwrap();
    let entry = thread.turns.get_mut("turn").unwrap();
    entry.slice.offset = 0;
    entry.slice.length = bytes.len() as u64;
    entry.slice.sha256 = crate::hash(bytes);
}
#[test]
fn measurement_visitor_stops_before_parsing_an_over_budget_next_record() {
    let root = tempfile::tempdir().unwrap();
    let (_, mut disk) = pair(root.path(), facts(vec![]));
    let first = disk.turn("thread", "turn").unwrap().measurements.remove(0);
    let first = serde_json::to_string(&first).unwrap();
    let bytes = format!("{{\"measurements\":[{first},null],\"operations\":[]}}");
    replace_slice(&mut disk, bytes.as_bytes());
    let cancel = AtomicBool::new(false);
    assert_eq!(
        code(
            disk.timing_evidence(
                target(),
                TimingReadBudget {
                    max_facts: 1,
                    ..Default::default()
                },
                &cancel
            )
            .unwrap_err()
        ),
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        code(
            disk.timing_evidence(target(), TimingReadBudget::default(), &cancel)
                .unwrap_err()
        ),
        "SNAPSHOT_CORRUPT"
    );
}
#[test]
fn canonical_operation_arrays_share_facts_bytes_and_metadata_budget() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![]);
    let op:Operation=serde_json::from_value(serde_json::json!({"id":"op","threadId":"thread","turnId":"turn","callId":"native","kind":"tool","name":"read_file","sequence":1,"timePrecision":"unknown","status":"running","outcomeConflict":false,"evidence":[]})).unwrap();
    data.operations = vec![Arc::new(op)];
    let (memory, disk) = pair(root.path(), data);
    let cancel = AtomicBool::new(false);
    let a = memory
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    let b = disk
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    assert_eq!(a.coverage, b.coverage);
    assert_eq!(a.coverage.facts, 2);
    assert_eq!(a.measurements.len(), 1);
    assert_eq!(a.operations, b.operations);
    assert_eq!(a.operations.len(), 1);
    for snapshot in [&memory, &disk] {
        assert_eq!(
            code(
                snapshot
                    .timing_evidence(
                        target(),
                        TimingReadBudget {
                            max_facts: 1,
                            ..Default::default()
                        },
                        &cancel
                    )
                    .unwrap_err()
            ),
            "RESOURCE_LIMIT"
        );
    }
}

fn in_domain(base: Arc<Event>, source: &str, file: &str, generation: &str) -> Arc<Event> {
    let position = Position {
        source_instance_id: source.into(),
        file_id: file.into(),
        generation: generation.into(),
        ..base.position().clone()
    };
    Arc::new(
        Event::new(
            position,
            base.thread_id().map(str::to_owned),
            base.turn_id().map(str::to_owned),
            base.time().clone(),
            base.gaps().to_vec(),
            base.payload().clone(),
        )
        .unwrap(),
    )
}
#[test]
fn overlapping_physical_selections_charge_each_block_once_per_bucket() {
    let root = tempfile::tempdir().unwrap();
    let mut events = vec![];
    for file in ["a", "c"] {
        for offset in [10, 100] {
            events.push(in_domain(
                event(offset, Some("thread"), Some("turn"), false),
                "source",
                file,
                "generation",
            ));
        }
    }
    for file in ["a", "b", "c"] {
        events.push(in_domain(
            event(50, None, None, true),
            "source",
            file,
            "generation",
        ));
    }
    for file in ["a", "c"] {
        events.push(in_domain(
            event(60, Some("thread"), None, true),
            "source",
            file,
            "generation",
        ));
    }
    events.push(in_domain(event(50, None, None, true), "source", "a", "old"));
    events.push(in_domain(
        event(50, None, None, true),
        "other-source",
        "a",
        "generation",
    ));
    let (memory, disk) = pair(root.path(), facts(events));
    let cancel = AtomicBool::new(false);
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget {
                    max_facts: 12,
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap();
        // Both unassigned buckets have chunk index 0. Global index 0 intersects
        // two spans, but only its own single encoding is charged once.
        assert_eq!(out.coverage.event_blocks, 3);
        assert_eq!(out.coverage.facts, 12);
        assert_eq!(out.controls.len(), 4);
        assert_eq!(out.domains.len(), 2);
        assert!(
            out.controls
                .iter()
                .all(|e| e.position().generation.as_ref() == "generation"
                    && e.position().source_instance_id.as_ref() == "source"
                    && e.position().file_id.as_ref() != "b")
        );
    }
}
#[test]
fn selected_control_block_corruption_is_never_an_empty_success() {
    let root = tempfile::tempdir().unwrap();
    let (_, disk) = pair(
        root.path(),
        facts(vec![
            event(10, Some("thread"), Some("turn"), false),
            event(20, Some("thread"), Some("turn"), false),
            event(15, None, None, true),
        ]),
    );
    let partition = disk
        .event_partition(&EventTarget {
            thread_id: None,
            turn_id: None,
        })
        .unwrap()
        .unwrap();
    let path = disk.directory.join(&partition.chunks[0].file.file);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        code(
            disk.timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
        ),
        "SNAPSHOT_CORRUPT"
    );
}
#[test]
fn canonical_measurement_source_conflicts_are_rejected_in_both_modes() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![]);
    let mut row = fact().as_ref().clone();
    row.source_instance_id = "other-source".into();
    data.measurements = vec![Arc::new(row)];
    let (memory, disk) = pair(root.path(), data);
    for snapshot in [&memory, &disk] {
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
            "SNAPSHOT_CORRUPT"
        );
    }
}

#[test]
fn retained_generation_mismatch_and_failed_file_observation_are_public_coverage() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![event(1, Some("thread"), Some("turn"), false)]);
    data.watermarks.push(SourceWatermark {
        original_source: None,
        format_version: WATERMARK_FORMAT_VERSION,
        source_instance_id: "source".into(),
        file_id: "file".into(),
        generation: Some("replacement-generation".into()),
        committed_offset: 10,
        observed_bytes: None,
        observed_at: "2026-10-01T00:00:00Z".into(),
        state: WatermarkState::Failed,
        issue_codes: vec![WatermarkIssue::SourceUnreadable],
    });
    data.sources.push(SourceReport {
        source: SourceInstance {
            id: "source".into(),
            agent_kind: "codex".into(),
            root: "/synthetic/source".into(),
        },
        adapter_version: "synthetic-adapter".into(),
        source_versions: vec![],
        capabilities: Capabilities {
            usage: true,
            ..Default::default()
        },
        status: "partial".into(),
        files_read: 1,
        bytes_read: 1,
        issues: vec![],
    });
    let (memory, disk) = pair(root.path(), data);
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(!out.domains[0].generation_matches);
        assert_eq!(
            out.domains[0].watermark.unwrap().state,
            WatermarkState::Failed
        );
        assert!(std::ptr::eq(
            out.source.unwrap(),
            &snapshot.manifest.sources[0]
        ));
        assert!(out.source.unwrap().capabilities.usage);
        assert_eq!(out.events.len(), 1); // Failure/replacement never deletes retained observations.
    }
}

fn operation(id: &str) -> Arc<Operation> {
    Arc::new(
        serde_json::from_value(serde_json::json!({
            "id":id,"threadId":"thread","turnId":"turn","callId":"native-call",
            "kind":"skillRead","name":"read_file","path":"/synthetic/skills/query/SKILL.md",
            "sequence":1,"timePrecision":"unknown","status":"failed","outcomeConflict":false,"evidence":[]
        }))
        .unwrap(),
    )
}
#[test]
fn operations_only_keep_canonical_rows_replays_missing_identity_and_time() {
    let root = tempfile::tempdir().unwrap();
    let replay = operation("canonical-operation");
    let mut unknown = replay.as_ref().clone();
    unknown.id = "anonymous-record".into();
    unknown.call_id = None;
    let mut data = facts(vec![]);
    data.measurements.clear();
    data.operations = vec![replay.clone(), replay.clone(), Arc::new(unknown)];
    let expected = data.operations.clone();
    let (memory, disk) = pair(root.path(), data);
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(out.measurements.is_empty());
        assert!(out.events.is_empty());
        assert!(out.domains.is_empty());
        assert_eq!(out.operations, expected);
        assert_eq!(out.coverage.facts, 3);
        assert!(out.operations[2].call_id.is_none());
        assert!(out.operations[2].timestamp.is_none());
    }
}
#[test]
fn operation_thread_and_turn_are_validated_in_both_read_modes() {
    for mismatch in ["thread", "turn", "missing_turn"] {
        let root = tempfile::tempdir().unwrap();
        let mut data = facts(vec![]);
        data.operations = vec![operation("operation")];
        let (mut memory, mut disk) = pair(root.path(), data);
        let mut invalid = operation("operation").as_ref().clone();
        match mismatch {
            "thread" => invalid.thread_id = "foreign-thread".into(),
            "turn" => invalid.turn_id = Some("foreign-turn".into()),
            _ => invalid.turn_id = None,
        }
        Arc::get_mut(memory.memory_turns.as_mut().unwrap())
            .unwrap()
            .get_mut(&("thread".into(), "turn".into()))
            .unwrap()
            .operations = vec![Arc::new(invalid.clone())];
        let mut slice = disk.turn("thread", "turn").unwrap();
        slice.operations = vec![invalid];
        replace_slice(&mut disk, &serde_json::to_vec(&slice).unwrap());
        for snapshot in [&memory, &disk] {
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
                "SNAPSHOT_CORRUPT",
                "{mismatch}"
            );
        }
    }
}
#[test]
fn operations_and_events_share_the_complete_target_budget() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![event(1, Some("thread"), Some("turn"), false)]);
    data.operations = vec![operation("operation")];
    let (memory, disk) = pair(root.path(), data);
    let cancel = AtomicBool::new(false);
    let a = memory
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    let b = disk
        .timing_evidence(target(), TimingReadBudget::default(), &cancel)
        .unwrap();
    assert_eq!(a.coverage, b.coverage);
    assert_eq!(a.coverage.facts, 3);
    for snapshot in [&memory, &disk] {
        for budget in [
            TimingReadBudget {
                max_facts: 2,
                ..Default::default()
            },
            TimingReadBudget {
                max_bytes: a.coverage.bytes - 1,
                ..Default::default()
            },
            TimingReadBudget {
                max_metadata: a.coverage.metadata - 1,
                ..Default::default()
            },
        ] {
            assert_eq!(
                code(
                    snapshot
                        .timing_evidence(target(), budget, &cancel)
                        .unwrap_err()
                ),
                "RESOURCE_LIMIT"
            );
        }
    }
}
#[test]
fn operation_seed_stops_before_parsing_an_over_budget_or_metadata_record() {
    let root = tempfile::tempdir().unwrap();
    let (_, mut disk) = pair(root.path(), facts(vec![]));
    let op = serde_json::to_string(&operation("operation")).unwrap();
    replace_slice(
        &mut disk,
        format!("{{\"measurements\":[],\"operations\":[{op},null]}}").as_bytes(),
    );
    let cancel = AtomicBool::new(false);
    let mut meter = Meter::new(TimingReadBudget::default(), &cancel);
    meter.work(1).unwrap(); // The first thread entry locates the owner.
    meter
        .work(
            disk.manifest.threads[0]
                .turns
                .len()
                .checked_ilog2()
                .unwrap_or(0) as usize
                + 1,
        )
        .unwrap();
    meter
        .partition(&disk, &EventTarget::turn("thread", "turn"))
        .unwrap();
    let metadata_before_operations = meter.coverage.metadata;
    for budget in [
        TimingReadBudget {
            max_facts: 1,
            ..Default::default()
        },
        TimingReadBudget {
            max_metadata: metadata_before_operations + 1,
            ..Default::default()
        },
    ] {
        assert_eq!(
            code(disk.timing_evidence(target(), budget, &cancel).unwrap_err()),
            "RESOURCE_LIMIT"
        );
    }
    assert_eq!(
        code(
            disk.timing_evidence(target(), TimingReadBudget::default(), &cancel)
                .unwrap_err()
        ),
        "SNAPSHOT_CORRUPT"
    );
}
#[test]
fn canonical_turn_reads_ignore_unrelated_turn_and_ledger_damage() {
    let root = tempfile::tempdir().unwrap();
    let mut data = facts(vec![]);
    let expected = operation("target-operation");
    let mut other = operation("other-operation").as_ref().clone();
    other.turn_id = Some("other-turn".into());
    data.operations = vec![expected.clone(), Arc::new(other)];
    let (mut memory, disk) = pair(root.path(), data);
    let other_slice = &disk.manifest.threads[0].turns["other-turn"].slice;
    let path = disk.directory.join(&disk.manifest.threads[0].file.file);
    let mut bytes = fs::read(&path).unwrap();
    bytes[other_slice.offset as usize] ^= 1;
    fs::write(path, bytes).unwrap();
    fs::write(
        disk.directory.join(&disk.manifest.ledger.file),
        b"invalid ledger",
    )
    .unwrap();
    Arc::get_mut(memory.memory_turns.as_mut().unwrap())
        .unwrap()
        .get_mut(&("thread".into(), "other-turn".into()))
        .unwrap()
        .operations = vec![Arc::new(Operation {
        text_result: None,
        thread_id: "foreign".into(),
        ..expected.as_ref().clone()
    })];
    for snapshot in [&memory, &disk] {
        let out = snapshot
            .timing_evidence(
                target(),
                TimingReadBudget::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(out.operations, vec![expected.clone()]);
        assert_eq!(out.measurements.len(), 1);
    }
}
