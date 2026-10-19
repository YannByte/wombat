//! Independent event partition, immutable cursor and failure fixtures.
use super::*;
use crate::session_events::{Event, LifecycleKind, Payload, Phase, Position, Time};
use std::sync::atomic::AtomicBool;

fn event(offset: u64, thread: Option<&str>, turn: Option<&str>) -> Arc<Event> {
    Arc::new(
        Event::new(
            Position {
                source_instance_id: "synthetic-source".into(),
                file_id: "synthetic-file".into(),
                generation: "synthetic-generation".into(),
                byte_offset: offset,
                ordinal: 0,
            },
            thread.map(str::to_owned),
            turn.map(str::to_owned),
            Time::from_source(None).0,
            vec![],
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
        events,
        ..Default::default()
    }
}
fn offsets(events: &[Arc<Event>]) -> Vec<u64> {
    events.iter().map(|e| e.position().byte_offset).collect()
}
fn budget() -> EventReadBudget {
    EventReadBudget::default()
}
fn active() -> AtomicBool {
    AtomicBool::new(false)
}
fn live(root: &Path, facts: Collected, id: &str) -> Snapshot {
    memory(
        facts,
        id.into(),
        crate::pricing_sync::current_at(root).unwrap(),
        None,
    )
    .unwrap()
}
fn target(thread: Option<&str>, turn: Option<&str>) -> EventTarget {
    EventTarget {
        thread_id: thread.map(str::to_owned),
        turn_id: turn.map(str::to_owned),
    }
}
#[test]
fn exact_partitions_preserve_unattributed_empty_and_cross_thread_facts() {
    let root = tempfile::tempdir().unwrap();
    let facts = facts(vec![
        event(30, Some("b"), Some("same")),
        event(10, None, None),
        event(21, Some("a"), Some("same")),
        event(20, Some("a"), None),
        event(22, Some("a"), Some("unassigned")),
    ]);
    let memory = live(root.path(), facts.clone(), "live:partition");
    let disk = save_at(root.path(), facts).unwrap();
    for snapshot in [&memory, &disk] {
        assert_eq!(
            offsets(
                &snapshot
                    .events_for_target(&target(None, None), budget(), &active())
                    .unwrap()
            ),
            vec![10]
        );
        assert_eq!(
            offsets(
                &snapshot
                    .events_for_target(&target(Some("a"), None), budget(), &active())
                    .unwrap()
            ),
            vec![20]
        );
        assert_eq!(
            offsets(
                &snapshot
                    .events_for_target(&EventTarget::turn("a", "unassigned"), budget(), &active())
                    .unwrap()
            ),
            vec![22]
        );
        assert_eq!(
            offsets(
                &snapshot
                    .events_for_target(&EventTarget::turn("a", "same"), budget(), &active())
                    .unwrap()
            ),
            vec![21]
        );
        assert_eq!(
            offsets(
                &snapshot
                    .events_for_target(&EventTarget::turn("b", "same"), budget(), &active())
                    .unwrap()
            ),
            vec![30]
        );
        let empty = snapshot
            .event_page(
                &EventTarget::turn("missing", "missing"),
                200,
                None,
                budget(),
                &active(),
            )
            .unwrap();
        assert_eq!(empty.total, 0);
        assert!(empty.events.is_empty());
        assert!(empty.next_cursor.is_none());
        assert_eq!(snapshot.events().unwrap().len(), 5);
    }
    let empty = save_at(root.path(), Collected::default()).unwrap();
    assert!(empty.manifest.events.partitions.is_empty());
    assert!(empty.events().unwrap().is_empty());
}
#[test]
fn page_order_and_cursors_are_bound_to_target_fixed_view_and_index() {
    let root = tempfile::tempdir().unwrap();
    let facts = facts(
        (0..405)
            .rev()
            .map(|i| event(i, Some("thread"), Some("turn")))
            .collect(),
    );
    let memory = live(root.path(), facts.clone(), "live:pages");
    let disk = save_at(root.path(), facts.clone()).unwrap();
    let next = save_at(root.path(), facts).unwrap();
    let target = EventTarget::turn("thread", "turn");
    for snapshot in [&memory, &disk] {
        let first = snapshot
            .event_page(&target, 199, None, budget(), &active())
            .unwrap();
        assert_eq!(first.total, 405);
        assert_eq!(offsets(&first.events), (0..199).collect::<Vec<_>>());
        let cursor = first.next_cursor.unwrap();
        let second = snapshot
            .event_page(&target, 200, Some(&cursor), budget(), &active())
            .unwrap();
        assert_eq!(offsets(&second.events), (199..399).collect::<Vec<_>>());
        let last = snapshot
            .event_page(
                &target,
                200,
                second.next_cursor.as_ref(),
                budget(),
                &active(),
            )
            .unwrap();
        assert_eq!(offsets(&last.events), (399..405).collect::<Vec<_>>());
        assert!(last.next_cursor.is_none());
        assert!(
            snapshot
                .event_page(
                    &EventTarget::turn("other", "turn"),
                    1,
                    Some(&cursor),
                    budget(),
                    &active()
                )
                .is_err()
        );
        assert!(
            next.event_page(&target, 1, Some(&cursor), budget(), &active())
                .is_err()
        );
        let mut raw = serde_json::to_value(&cursor).unwrap();
        raw["nextOffset"] = serde_json::json!(201);
        let altered: EventCursor = serde_json::from_value(raw).unwrap();
        assert!(
            snapshot
                .event_page(&target, 1, Some(&altered), budget(), &active())
                .is_err()
        );
        assert!(
            snapshot
                .event_page(&target, 0, None, budget(), &active())
                .is_err()
        );
        assert!(
            snapshot
                .event_page(&target, 201, None, budget(), &active())
                .is_err()
        );
    }
    let cursor = disk
        .event_page(&target, 1, None, budget(), &active())
        .unwrap()
        .next_cursor
        .unwrap();
    let mut changed = load_at(root.path(), Some(&disk.manifest.snapshot_ref.snapshot_id)).unwrap();
    Arc::make_mut(&mut changed.manifest.events.partitions[0]).sha256 = "altered".into();
    assert!(
        changed
            .event_page(&target, 1, Some(&cursor), budget(), &active())
            .is_err()
    );
}
#[test]
fn target_and_page_reads_do_not_touch_unrelated_or_later_event_blocks() {
    let root = tempfile::tempdir().unwrap();
    let mut events: Vec<_> = (0..405)
        .map(|i| event(i, Some("a"), Some("target")))
        .collect();
    events.push(event(500, Some("b"), Some("unrelated")));
    let snapshot = save_at(root.path(), facts(events)).unwrap();
    let target_partition = &snapshot.manifest.events.partitions[0];
    fs::remove_file(
        snapshot
            .directory
            .join(&target_partition.chunks[2].file.file),
    )
    .unwrap();
    fs::remove_file(
        snapshot
            .directory
            .join(&snapshot.manifest.events.partitions[1].chunks[0].file.file),
    )
    .unwrap();
    fs::remove_file(snapshot.directory.join(&snapshot.manifest.ledger.file)).unwrap();
    let page = snapshot
        .event_page(
            &EventTarget::turn("a", "target"),
            200,
            None,
            budget(),
            &active(),
        )
        .unwrap();
    assert_eq!(offsets(&page.events), (0..200).collect::<Vec<_>>());
    assert!(
        snapshot
            .events_for_target(&EventTarget::turn("a", "target"), budget(), &active())
            .is_err()
    );
}
#[test]
fn budgets_reject_before_read_and_cancellation_never_returns_partial_events() {
    let root = tempfile::tempdir().unwrap();
    let facts = facts(
        (0..201)
            .map(|i| event(i, Some("a"), Some("turn")))
            .collect(),
    );
    let memory = live(root.path(), facts.clone(), "live:budget");
    let disk = save_at(root.path(), facts).unwrap();
    let target = EventTarget::turn("a", "turn");
    for snapshot in [&memory, &disk] {
        assert!(
            snapshot
                .events_for_target(
                    &target,
                    EventReadBudget {
                        max_facts: 200,
                        ..budget()
                    },
                    &active()
                )
                .unwrap_err()
                .to_string()
                .contains("资源预算")
        );
        assert!(
            snapshot
                .event_page(
                    &target,
                    200,
                    None,
                    EventReadBudget {
                        max_facts: 199,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
        assert!(
            snapshot
                .event_page(
                    &target,
                    1,
                    None,
                    EventReadBudget {
                        max_bytes: 1,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
        assert!(
            snapshot
                .events_for_target(&target, budget(), &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("已取消")
        );
        assert!(
            snapshot
                .event_page(&target, 1, None, budget(), &AtomicBool::new(true))
                .is_err()
        );
    }
    fs::remove_file(
        disk.directory
            .join(&disk.manifest.events.partitions[0].chunks[0].file.file),
    )
    .unwrap();
    assert!(
        disk.events_for_target(
            &target,
            EventReadBudget {
                max_facts: 200,
                ..budget()
            },
            &active()
        )
        .unwrap_err()
        .to_string()
        .contains("资源预算")
    );
    let mut too_large =
        load_at(root.path(), Some(&disk.manifest.snapshot_ref.snapshot_id)).unwrap();
    Arc::make_mut(&mut too_large.manifest.events.partitions[0]).count = MAX_TARGET_EVENTS + 1;
    assert!(
        too_large
            .events_for_target(
                &target,
                EventReadBudget {
                    max_facts: usize::MAX,
                    ..budget()
                },
                &active()
            )
            .unwrap_err()
            .to_string()
            .contains("资源预算")
    );
}
#[test]
fn byte_budgets_include_utf8_and_array_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let source = event(0, Some("线程"), Some("轮次"));
    let memory = live(root.path(), facts(vec![source.clone()]), "live:bytes");
    let disk = save_at(root.path(), facts(vec![source.clone()])).unwrap();
    let target = EventTarget::turn("线程", "轮次");
    let bytes =
        serde_json::to_vec(&serde_json::json!({"version":disk.manifest.events.version,"target":target,"events":[source]}))
            .unwrap()
            .len() as u64;
    assert_eq!(disk.manifest.events.partitions[0].chunks[0].bytes, bytes);
    for snapshot in [&memory, &disk] {
        assert_eq!(
            snapshot
                .events_for_target(
                    &target,
                    EventReadBudget {
                        max_bytes: bytes,
                        ..budget()
                    },
                    &active()
                )
                .unwrap()
                .len(),
            1
        );
        assert!(
            snapshot
                .events_for_target(
                    &target,
                    EventReadBudget {
                        max_bytes: bytes - 1,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
        assert!(
            snapshot
                .event_page(
                    &EventTarget::turn("missing", "missing"),
                    1,
                    None,
                    EventReadBudget {
                        max_bytes: 1,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
        assert!(
            snapshot
                .events_for_target(
                    &EventTarget::turn("missing", "missing"),
                    EventReadBudget {
                        max_bytes: 1,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
    }
}

#[test]
fn hashes_versions_duplicate_positions_and_index_corruption_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let duplicated = event(0, Some("a"), Some("turn"));
    assert!(
        save_at(
            root.path(),
            facts(vec![duplicated.clone(), duplicated.clone()])
        )
        .is_err()
    );
    assert!(live_result(root.path(), facts(vec![duplicated.clone(), duplicated])).is_err());
    let snapshot = save_at(root.path(), facts(vec![event(0, Some("a"), Some("turn"))])).unwrap();
    let target = EventTarget::turn("a", "turn");
    let path = snapshot
        .directory
        .join(&snapshot.manifest.events.partitions[0].chunks[0].file.file);
    let original = fs::read(&path).unwrap();
    let mut changed = original.clone();
    changed[10] ^= 1;
    fs::write(&path, changed).unwrap();
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .unwrap_err()
            .to_string()
            .contains("校验失败")
    );
    fs::write(&path, original).unwrap();
    let manifest_path = snapshot.directory.join("manifest.json");
    let raw = serde_json::to_value(&snapshot.manifest).unwrap();
    for version in [0, 1, 3] {
        let mut changed = raw.clone();
        changed["events"]["version"] = serde_json::json!(version);
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            load_at(root.path(), None)
                .err()
                .unwrap()
                .to_string()
                .contains("不支持此事件索引版本")
        );
    }
    let mut old_shape = raw.clone();
    old_shape["events"] = serde_json::json!({"file":"events.json","sha256":"hash"});
    fs::write(&manifest_path, serde_json::to_vec(&old_shape).unwrap()).unwrap();
    assert!(load_at(root.path(), None).is_err());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&manifest_path).unwrap()).unwrap(),
        old_shape
    );
    let mut index = snapshot.manifest.events.clone();
    index.partitions.push(index.partitions[0].clone());
    assert!(super::events::validate_index(&index).is_err());
    let mut index = snapshot.manifest.events.clone();
    Arc::make_mut(&mut index.partitions[0]).count += 1;
    assert!(super::events::validate_index(&index).is_err());
}
fn live_result(root: &Path, facts: Collected) -> Result<Snapshot> {
    memory(
        facts,
        "live:duplicate".into(),
        crate::pricing_sync::current_at(root).unwrap(),
        None,
    )
}

fn rewrite_block(snapshot: &mut Snapshot, block: serde_json::Value) {
    let chunk = &mut Arc::make_mut(&mut snapshot.manifest.events.partitions[0]).chunks[0];
    let bytes = serde_json::to_vec(&block).unwrap();
    fs::write(snapshot.directory.join(&chunk.file.file), &bytes).unwrap();
    chunk.bytes = bytes.len() as u64;
    chunk.file.sha256 = crate::hash(&bytes);
}
#[test]
fn valid_hash_cannot_hide_unknown_block_or_event_version_or_scope_mismatch() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot = save_at(
        root.path(),
        facts(vec![
            event(0, Some("a"), Some("turn")),
            event(1, Some("a"), Some("turn")),
        ]),
    )
    .unwrap();
    let path = snapshot
        .directory
        .join(&snapshot.manifest.events.partitions[0].chunks[0].file.file);
    let original: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let target = EventTarget::turn("a", "turn");
    let mut changed = original.clone();
    changed["version"] = serde_json::json!(3);
    rewrite_block(&mut snapshot, changed);
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .unwrap_err()
            .to_string()
            .contains("不支持此事件块版本")
    );
    let mut changed = original.clone();
    changed["events"][0]["version"] = serde_json::json!(crate::session_events::EVENT_VERSION + 1);
    rewrite_block(&mut snapshot, changed);
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .unwrap_err()
            .to_string()
            .contains("不支持此事件版本")
    );
    let mut changed = original.clone();
    changed["events"][0]["threadId"] = serde_json::json!("b");
    rewrite_block(&mut snapshot, changed);
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .is_err()
    );
    let mut changed = original.clone();
    changed["events"][0] = changed["events"][1].clone();
    rewrite_block(&mut snapshot, changed);
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .is_err()
    );
    let mut changed = original;
    changed["extraFutureField"] = serde_json::json!(true);
    rewrite_block(&mut snapshot, changed);
    assert!(
        snapshot
            .events_for_target(&target, budget(), &active())
            .is_err()
    );
}
#[test]
fn index_rejects_cross_chunk_duplicates_even_when_its_digest_is_updated() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = save_at(
        root.path(),
        facts(
            (0..201)
                .map(|i| event(i, Some("a"), Some("turn")))
                .collect(),
        ),
    )
    .unwrap();
    for mutation in 0..5 {
        let mut index = snapshot.manifest.events.clone();
        let partition = Arc::make_mut(&mut index.partitions[0]);
        match mutation {
            0 => partition.chunks[1].first = partition.chunks[0].last.clone(),
            1 => partition.chunks[1].offset = 0,
            2 => partition.chunks[1].file.file = partition.chunks[0].file.file.clone(),
            3 => partition.chunks[0].bytes = 4 * 1024 * 1024 + 1,
            4 => partition.count += 1,
            _ => unreachable!(),
        }
        partition.sha256 = crate::hash(
            serde_json::to_vec(&(
                &partition.target,
                partition.count,
                &partition.chunks,
                &partition.native_boundary,
            ))
            .unwrap(),
        );
        assert!(super::events::validate_index(&index).is_err());
    }
    let a = event(0, Some("a"), Some("turn"));
    let b = event(0, Some("b"), Some("turn"));
    assert_eq!(a.id(), b.id());
    assert!(save_at(root.path(), facts(vec![a, b])).is_err());
}
#[test]
fn source_position_order_ignores_input_order_and_wall_clock_ties() {
    let root = tempfile::tempdir().unwrap();
    let create = |file: &str, generation: &str, offset: u64, ordinal: u32| {
        Arc::new(
            Event::new(
                Position {
                    source_instance_id: "source".into(),
                    file_id: file.into(),
                    generation: generation.into(),
                    byte_offset: offset,
                    ordinal,
                },
                Some("a".into()),
                Some("turn".into()),
                Time::from_source(Some("2026-10-04T00:00:00Z")).0,
                vec![],
                Payload::Activity {
                    activity: crate::session_events::ActivityKind::Assistant,
                },
            )
            .unwrap(),
        )
    };
    let facts = facts(vec![
        create("b", "a", 1, 0),
        create("a", "b", 1, 0),
        create("a", "a", 2, 1),
        create("a", "a", 2, 0),
    ]);
    let memory = live(root.path(), facts.clone(), "live:ordered");
    let disk = save_at(root.path(), facts).unwrap();
    for snapshot in [&memory, &disk] {
        let events = snapshot
            .events_for_target(&EventTarget::turn("a", "turn"), budget(), &active())
            .unwrap();
        let order: Vec<_> = events
            .iter()
            .map(|e| {
                (
                    e.position().file_id.as_ref(),
                    e.position().generation.as_ref(),
                    e.position().byte_offset,
                    e.position().ordinal,
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                ("a", "a", 2, 0),
                ("a", "a", 2, 1),
                ("a", "b", 1, 0),
                ("b", "a", 1, 0)
            ]
        );
    }
}
#[test]
fn oversized_single_fact_does_not_publish_in_memory_or_disk() {
    let root = tempfile::tempdir().unwrap();
    let prior = save_at(root.path(), Collected::default()).unwrap();
    let large = Arc::new(
        Event::new(
            event(0, Some("a"), Some("turn")).position().clone(),
            Some("a".into()),
            Some("turn".into()),
            Time::from_source(None).0,
            vec![],
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Turn,
                phase: Phase::Completed,
                native_id: Some("x".repeat(4 * 1024 * 1024)),
                duration_ms: None,
                first_token_ms: None,
            },
        )
        .unwrap(),
    );
    assert!(
        live_result(root.path(), facts(vec![large.clone()]))
            .err()
            .unwrap()
            .to_string()
            .contains("超过4 MiB")
    );
    assert!(
        save_at(root.path(), facts(vec![large]))
            .err()
            .unwrap()
            .to_string()
            .contains("超过4 MiB")
    );
    assert_eq!(
        load_at(root.path(), None)
            .unwrap()
            .manifest
            .snapshot_ref
            .snapshot_id,
        prior.manifest.snapshot_ref.snapshot_id
    );
}

#[test]
fn invalid_targets_and_unsafe_event_paths_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let facts = facts(vec![event(0, Some("a"), Some("turn"))]);
    let memory = live(root.path(), facts.clone(), "live:paths");
    let mut disk = save_at(root.path(), facts).unwrap();
    for snapshot in [&memory, &disk] {
        for target in [
            EventTarget {
                thread_id: None,
                turn_id: Some("turn".into()),
            },
            EventTarget::turn("", "turn"),
            EventTarget::turn("a", ""),
        ] {
            assert!(
                snapshot
                    .events_for_target(&target, budget(), &active())
                    .is_err()
            );
            assert!(
                snapshot
                    .event_page(&target, 1, None, budget(), &active())
                    .is_err()
            );
        }
    }
    Arc::make_mut(&mut disk.manifest.events.partitions[0]).chunks[0]
        .file
        .file = "../outside.json".into();
    assert!(
        disk.events_for_target(&EventTarget::turn("a", "turn"), budget(), &active())
            .unwrap_err()
            .to_string()
            .contains("非法快照分片路径")
    );
}
#[cfg(unix)]
#[test]
fn event_shard_symlinks_cannot_escape_the_generation() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = save_at(root.path(), facts(vec![event(0, Some("a"), Some("turn"))])).unwrap();
    let path = snapshot
        .directory
        .join(&snapshot.manifest.events.partitions[0].chunks[0].file.file);
    let outside = root.path().join("synthetic-outside.json");
    fs::rename(&path, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(
        snapshot
            .events_for_target(&EventTarget::turn("a", "turn"), budget(), &active())
            .unwrap_err()
            .to_string()
            .contains("符号链接")
    );
}

#[test]
fn byte_and_row_limits_pack_variable_blocks_identically_for_memory_and_disk() {
    let root = tempfile::tempdir().unwrap();
    let large = |offset| {
        Arc::new(
            Event::new(
                event(offset, Some("a"), Some("turn")).position().clone(),
                Some("a".into()),
                Some("turn".into()),
                Time::from_source(None).0,
                vec![],
                Payload::Lifecycle {
                    lifecycle: LifecycleKind::Turn,
                    phase: Phase::Completed,
                    native_id: Some("中".repeat(700_000)),
                    duration_ms: None,
                    first_token_ms: None,
                },
            )
            .unwrap(),
        )
    };
    let facts = facts(vec![large(0), large(1), event(2, Some("a"), Some("turn"))]);
    let memory = live(root.path(), facts.clone(), "live:variable");
    let disk = save_at(root.path(), facts).unwrap();
    assert_eq!(
        serde_json::to_value(&memory.manifest.events).unwrap(),
        serde_json::to_value(&disk.manifest.events).unwrap()
    );
    let partition = &disk.manifest.events.partitions[0];
    assert_eq!(partition.chunks.len(), 2);
    assert_eq!(
        partition
            .chunks
            .iter()
            .map(|c| (c.offset, c.count))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 2)]
    );
    let target = EventTarget::turn("a", "turn");
    for snapshot in [&memory, &disk] {
        let page = snapshot
            .event_page(
                &target,
                1,
                None,
                EventReadBudget {
                    max_bytes: partition.chunks[0].bytes,
                    ..budget()
                },
                &active(),
            )
            .unwrap();
        assert_eq!(offsets(&page.events), vec![0]);
        let cursor = page.next_cursor.unwrap();
        let page = snapshot
            .event_page(
                &target,
                2,
                Some(&cursor),
                EventReadBudget {
                    max_bytes: partition.chunks[1].bytes,
                    ..budget()
                },
                &active(),
            )
            .unwrap();
        assert_eq!(offsets(&page.events), vec![1, 2]);
        assert!(page.next_cursor.is_none());
        let combined = partition.chunks.iter().map(|c| c.bytes).sum();
        assert_eq!(
            snapshot
                .events_for_target(
                    &target,
                    EventReadBudget {
                        max_bytes: combined,
                        ..budget()
                    },
                    &active()
                )
                .unwrap()
                .len(),
            3
        );
        assert!(
            snapshot
                .events_for_target(
                    &target,
                    EventReadBudget {
                        max_bytes: combined - 1,
                        ..budget()
                    },
                    &active()
                )
                .is_err()
        );
    }
}

#[test]
fn future_index_fields_do_not_hide_an_unsupported_version() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = save_at(root.path(), Collected::default()).unwrap();
    let manifest_path = snapshot.directory.join("manifest.json");
    let original = serde_json::to_value(&snapshot.manifest).unwrap();
    let future_index_version = snapshot.manifest.events.version + 1;
    for index in [
        serde_json::json!({"version":future_index_version,"partitions":[],"futureField":{"private":"uninterpreted"}}),
        serde_json::json!({"version":future_index_version,"futureShape":true}),
    ] {
        let mut changed = original.clone();
        changed["events"] = index;
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let error = load_at(root.path(), None).err().unwrap();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "UNSUPPORTED_VERSION"
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(&manifest_path).unwrap())
                .unwrap(),
            changed
        );
    }
    for index in [
        serde_json::json!({"version":snapshot.manifest.events.version,"partitions":[],"futureField":true}),
        serde_json::json!({"partitions":[]}),
        serde_json::json!({"version":"1","partitions":[]}),
    ] {
        let mut changed = original.clone();
        changed["events"] = index;
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let error = load_at(root.path(), None).err().unwrap();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "SNAPSHOT_CORRUPT"
        );
    }
}

#[test]
fn future_block_and_fact_fields_are_version_errors_before_current_shape_parsing() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot =
        save_at(root.path(), facts(vec![event(0, Some("a"), Some("turn"))])).unwrap();
    let path = snapshot
        .directory
        .join(&snapshot.manifest.events.partitions[0].chunks[0].file.file);
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let target = EventTarget::turn("a", "turn");
    let mut future_block = original.clone();
    future_block["version"] = serde_json::json!(snapshot.manifest.events.version + 1);
    future_block["futureField"] = serde_json::json!({"private":"uninterpreted"});
    future_block.as_object_mut().unwrap().remove("events");
    let mut future_fact = original.clone();
    future_fact["events"][0]["version"] =
        serde_json::json!(crate::session_events::EVENT_VERSION + 1);
    future_fact["events"][0]["futureField"] = serde_json::json!(true);
    future_fact["events"][0]["payload"] =
        serde_json::json!({"kind":"future_kind","private":"uninterpreted"});
    for changed in [future_block, future_fact] {
        rewrite_block(&mut snapshot, changed);
        let error = snapshot
            .events_for_target(&target, budget(), &active())
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "UNSUPPORTED_VERSION"
        );
    }
    let mut missing_block_header = original.clone();
    missing_block_header
        .as_object_mut()
        .unwrap()
        .remove("version");
    let mut missing_fact_header = original.clone();
    missing_fact_header["events"][0]
        .as_object_mut()
        .unwrap()
        .remove("version");
    let mut invalid_fact_header = original.clone();
    invalid_fact_header["events"][0]["version"] = serde_json::json!("1");
    let mut oversized_event_array = original.clone();
    oversized_event_array["events"] =
        serde_json::Value::Array(vec![original["events"][0].clone(); MAX_EVENT_PAGE_ROWS + 1]);
    for changed in [
        missing_block_header,
        missing_fact_header,
        invalid_fact_header,
        oversized_event_array,
    ] {
        rewrite_block(&mut snapshot, changed);
        let error = snapshot
            .events_for_target(&target, budget(), &active())
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "SNAPSHOT_CORRUPT"
        );
    }
}

#[test]
fn local_offset_locators_read_exact_fixed_pages_without_traversal() {
    let root = tempfile::tempdir().unwrap();
    let input = facts(
        (0..405)
            .rev()
            .map(|i| event(i, Some("thread"), Some("turn")))
            .collect(),
    );
    let memory = live(root.path(), input.clone(), "live:locators");
    let disk = save_at(root.path(), input.clone()).unwrap();
    let next = save_at(root.path(), input).unwrap();
    let target = EventTarget::turn("thread", "turn");
    for snapshot in [&memory, &disk] {
        assert!(
            snapshot
                .event_cursor_at_offset(&target, 0, &active())
                .unwrap()
                .is_none()
        );
        let cursor = snapshot
            .event_cursor_at_offset(&target, 400, &active())
            .unwrap()
            .unwrap();
        let page = snapshot
            .event_page(&target, 200, Some(&cursor), budget(), &active())
            .unwrap();
        assert_eq!(offsets(&page.events), [400, 401, 402, 403, 404]);
        assert_eq!(page.total, 405);
        assert!(
            snapshot
                .event_cursor_at_offset(&target, 405, &active())
                .is_err()
        );
        assert!(
            snapshot
                .event_cursor_at_offset(&EventTarget::turn("other", "turn"), 1, &active())
                .is_err()
        );
        assert!(
            snapshot
                .event_cursor_at_offset(&target, 200, &AtomicBool::new(true))
                .is_err()
        );
        assert!(
            next.event_page(&target, 200, Some(&cursor), budget(), &active())
                .is_err()
        );
        assert!(
            snapshot
                .event_page(
                    &EventTarget::turn("thread", "other"),
                    200,
                    Some(&cursor),
                    budget(),
                    &active()
                )
                .is_err()
        );
    }
}
