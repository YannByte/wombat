use super::*;
fn position() -> Position {
    Position {
        source_instance_id: "source".into(),
        file_id: "file".into(),
        generation: "generation".into(),
        byte_offset: 20,
        ordinal: 0,
    }
}
fn event() -> Event {
    Event::new(
        position(),
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(Some("2026-10-04T10:00:00.123+08:00")).0,
        vec![],
        Payload::Activity {
            activity: ActivityKind::Assistant,
        },
    )
    .unwrap()
}
#[test]
fn record_identity_separates_files_generations_offsets_and_ordinals() {
    let p = position();
    let id = p.event_id().unwrap();
    for changed in [
        Position {
            file_id: "other".into(),
            ..p.clone()
        },
        Position {
            generation: "replacement".into(),
            ..p.clone()
        },
        Position {
            byte_offset: 21,
            ..p.clone()
        },
        Position {
            ordinal: 1,
            ..p.clone()
        },
        Position {
            source_instance_id: "other".into(),
            ..p.clone()
        },
    ] {
        assert_ne!(id, changed.event_id().unwrap());
    }
    assert_eq!(id, p.event_id().unwrap());
    let a = Position {
        source_instance_id: "a:b".into(),
        file_id: "c".into(),
        ..p.clone()
    };
    let b = Position {
        source_instance_id: "a".into(),
        file_id: "b:c".into(),
        ..p
    };
    assert_ne!(a.event_id().unwrap(), b.event_id().unwrap());
}
#[test]
fn unknown_time_is_not_zero_and_invalid_time_is_an_explicit_gap() {
    let (missing, gap) = Time::from_source(None);
    assert!(missing.timestamp.is_none());
    assert!(gap.is_none());
    let (invalid, gap) = Time::from_source(Some("not-a-date"));
    assert!(invalid.timestamp.is_none());
    assert_eq!(gap, Some(Gap::InvalidTimestamp));
    let (known, _) = Time::from_source(Some("2026-10-04T10:00:00.123+08:00"));
    assert_eq!(
        known.timestamp.as_deref(),
        Some("2026-10-04T02:00:00.123000000Z")
    );
    assert_eq!(known.precision, Precision::Millisecond);
}
#[test]
fn current_envelope_round_trips_but_unknown_versions_and_changed_identity_fail() {
    let original = event();
    let json = serde_json::to_value(&original).unwrap();
    let restored: Event = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(restored.id(), original.id());
    for (field, value) in [
        ("version", serde_json::json!(EVENT_VERSION + 1)),
        ("id", serde_json::json!("forged")),
        ("raw", serde_json::json!("SYNTHETIC_PRIVATE_BODY")),
    ] {
        let mut changed = json.clone();
        changed[field] = value;
        assert!(serde_json::from_value::<Event>(changed).is_err());
    }
}
#[test]
fn payload_rejects_bodies_and_unknown_kinds() {
    let mut json = serde_json::to_value(event()).unwrap();
    json["payload"]["body"] = serde_json::json!("SYNTHETIC_PRIVATE_BODY");
    assert!(serde_json::from_value::<Event>(json).is_err());
    assert!(serde_json::from_str::<Payload>(r#"{"kind":"raw","text":"secret"}"#).is_err());
}
#[test]
fn source_constructor_rejects_unscoped_turn_and_false_precision() {
    assert!(
        Event::new(
            position(),
            None,
            Some("turn".into()),
            Time::from_source(None).0,
            vec![],
            Payload::Activity {
                activity: ActivityKind::Tool
            }
        )
        .is_err()
    );
    assert!(
        Event::new(
            position(),
            None,
            None,
            Time {
                timestamp: None,
                precision: Precision::Second
            },
            vec![],
            Payload::Activity {
                activity: ActivityKind::Tool
            }
        )
        .is_err()
    );
}

#[test]
fn payload_scope_cannot_be_rebound_to_another_turn() {
    let fact = Turn {
        id: "turn".into(),
        thread_id: "thread".into(),
        upstream_id: "native-turn".into(),
        ordinal: 1,
        started_at: None,
        ended_at: None,
        last_activity_at: None,
        status: "running".into(),
    };
    let original = Event::new(
        position(),
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        Payload::Turn {
            value: Box::new(fact),
            evidence: EvidenceRef {
                file: "synthetic".into(),
                line: 1,
            },
        },
    )
    .unwrap();
    let mut json = serde_json::to_value(original).unwrap();
    json["turnId"] = serde_json::json!("different");
    assert!(serde_json::from_value::<Event>(json).is_err());
}
#[test]
fn native_zero_duration_is_preserved_without_inventing_a_timestamp() {
    let event = Event::new(
        position(),
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        Payload::Lifecycle {
            lifecycle: LifecycleKind::ModelRequest,
            phase: Phase::Completed,
            native_id: Some("response".into()),
            duration_ms: Some(0),
            first_token_ms: None,
        },
    )
    .unwrap();
    let restored: Event = serde_json::from_slice(&serde_json::to_vec(&event).unwrap()).unwrap();
    assert!(restored.time().timestamp.is_none());
    assert!(matches!(
        restored.payload(),
        Payload::Lifecycle {
            duration_ms: Some(0),
            first_token_ms: None,
            ..
        }
    ));
}

#[test]
fn restored_item_retains_submillisecond_duration_and_rejects_unsafe_native_numbers() {
    let mut json = serde_json::to_value(event()).unwrap();
    json["payload"] = serde_json::json!({"kind":"item","item_kind":"command","native_id":"c","phase":"completed","started_at_ms":0,"completed_at_ms":1,"duration":{"secs":0,"nanos":1}});
    let restored: Event = serde_json::from_value(json.clone()).unwrap();
    assert!(
        matches!(restored.payload(), Payload::Item { duration: Some(d), .. } if d.secs == 0 && d.nanos == 1)
    );
    for (pointer, value) in [
        ("/payload/duration/nanos", 1_000_000_000u64),
        ("/payload/duration/secs", u64::MAX),
        ("/payload/completed_at_ms", MAX_SAFE_INTEGER + 1),
    ] {
        let mut malformed = json.clone();
        *malformed.pointer_mut(pointer).unwrap() = serde_json::json!(value);
        assert!(serde_json::from_value::<Event>(malformed).is_err());
    }
}

#[test]
fn safe_message_roundtrips_closed_enums_and_rejects_body_or_empty_identity() {
    let message = Event::new(
        position(),
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        Payload::Message {
            origin: MessageOrigin::AssistantVisible,
            presence: ContentPresence::Unknown,
            native_id: Some("item".into()),
            record_kind: MessageRecordKind::NativeSnapshot,
            record_phase: Phase::Completed,
            content_phase: ContentPhase::Unknown,
        },
    )
    .unwrap();
    let stored = serde_json::to_value(&message).unwrap();
    assert!(serde_json::from_value::<Event>(stored.clone()).is_ok());
    for (field, value) in [
        ("body", serde_json::json!("PRIVATE")),
        ("presence", serde_json::json!("future")),
        ("native_id", serde_json::json!("")),
    ] {
        let mut changed = stored.clone();
        changed["payload"][field] = value;
        assert!(serde_json::from_value::<Event>(changed).is_err());
    }
}

#[test]
fn independent_collection_time_is_required_and_does_not_change_identity_or_source_time() {
    let mut stored = serde_json::to_value(event()).unwrap();
    let first: Event = serde_json::from_value(stored.clone()).unwrap();
    stored["collectedAt"] = serde_json::json!("2026-10-06T12:00:00Z");
    let later: Event = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(first.id(), later.id());
    assert_eq!(first.time(), later.time());
    assert_ne!(first.collected_at(), later.collected_at());
    stored["time"] = serde_json::json!({"timestamp":null,"precision":"unknown"});
    let unknown: Event = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(unknown.time().timestamp, None);
    stored["collectedAt"] = serde_json::json!("invalid");
    assert!(serde_json::from_value::<Event>(stored.clone()).is_err());
    stored.as_object_mut().unwrap().remove("collectedAt");
    assert!(serde_json::from_value::<Event>(stored.clone()).is_err());
    stored["collectedAt"] = serde_json::json!("2026-10-06T12:00:00Z");
    stored["version"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Event>(stored).is_err());
}

#[test]
fn adjacent_sharing_survives_pool_exhaustion_without_changing_wire_values() {
    let mut pool = EventStrings::default();
    let mut prior = event();
    for n in 0..5000 {
        let mut current = event();
        current.0.thread_id = Some(format!("thread-{n}").into());
        current.0.turn_id = Some(format!("turn-{n}").into());
        pool.share(&mut current);
        prior = current;
    }
    // Deserialize to ensure the incoming allocations are independent.
    let expected = serde_json::to_vec(&prior).unwrap();
    let mut next: Event = serde_json::from_slice(&expected).unwrap();
    pool.share(&mut next);
    assert_eq!(serde_json::to_vec(&next).unwrap(), expected);
    for (a, b) in [
        (
            prior.0.thread_id.as_ref().unwrap(),
            next.0.thread_id.as_ref().unwrap(),
        ),
        (
            prior.0.turn_id.as_ref().unwrap(),
            next.0.turn_id.as_ref().unwrap(),
        ),
        (&prior.0.collected_at, &next.0.collected_at),
        (
            prior.0.time.timestamp.as_ref().unwrap(),
            next.0.time.timestamp.as_ref().unwrap(),
        ),
    ] {
        assert!(Arc::ptr_eq(a, b));
    }
    assert!(Arc::ptr_eq(&next.shared_id(), &next.0.id));
    let mut oversized: Arc<str> = "大".repeat(1024).into();
    pool.collected.share(&mut oversized);
    assert!(pool.collected.0.as_ref().unwrap().len() <= 1024);
    assert_eq!(oversized.as_ref(), "大".repeat(1024));
}

#[test]
fn scope_payload_sharing_preserves_canonical_wire_format_and_revisions() {
    let owned = Thread {
        id: "thread".to_owned(),
        agent_kind: "codex".into(),
        source_instance_id: "source".into(),
        upstream_id: "native".into(),
        title: Some("标题😀".into()),
        project: Some("/synthetic/project".into()),
        started_at: Some("2026-10-11T00:00:00.000000000Z".into()),
        last_activity_at: Some("2026-10-11T00:00:00.000000000Z".into()),
    };
    let expected = serde_json::to_vec(&owned).unwrap();
    let shared = owned.into_shared();
    assert_eq!(serde_json::to_vec(&shared).unwrap(), expected);
    let mut event = Event::new_at(
        position(),
        Some("thread".into()),
        None,
        Time::from_source(Some("2026-10-11T00:00:00Z")).0,
        vec![],
        Payload::Thread {
            value: Box::new(shared),
            project_path: Some("/synthetic/project".into()),
            evidence: EvidenceRef {
                file: "synthetic/file".into(),
                line: 1,
            },
        },
        "2026-10-11T00:00:01Z".into(),
    )
    .unwrap();
    let expected = serde_json::to_vec(&event).unwrap();
    let mut pool = EventStrings::default();
    pool.share(&mut event);
    let prior = event.clone();
    let mut later: Event = serde_json::from_slice(&expected).unwrap();
    pool.share(&mut later);
    if let (Payload::Thread { value: a, .. }, Payload::Thread { value: b, .. }) =
        (&event.0.payload, &mut later.0.payload)
    {
        assert!(Arc::ptr_eq(&a.id, &b.id));
        assert!(Arc::ptr_eq(
            a.project.as_ref().unwrap(),
            b.project.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            a.started_at.as_ref().unwrap(),
            event.0.time.timestamp.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            a.started_at.as_ref().unwrap(),
            a.last_activity_at.as_ref().unwrap()
        ));
        b.title = Some("changed".into());
    } else {
        panic!("expected thread payload");
    }
    assert_eq!(serde_json::to_vec(&event).unwrap(), expected);
    assert_eq!(serde_json::to_vec(&prior).unwrap(), expected);
    assert_ne!(serde_json::to_vec(&later).unwrap(), expected);
}
