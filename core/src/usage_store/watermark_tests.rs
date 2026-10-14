use super::*;
fn watermark() -> SourceWatermark {
    SourceWatermark {
        original_source: None,
        format_version: crate::adapters::contract::WATERMARK_FORMAT_VERSION,
        source_instance_id: "synthetic".into(),
        file_id: "file".into(),
        generation: Some("generation".into()),
        committed_offset: 10,
        observed_bytes: Some(15),
        observed_at: "2026-10-05T00:00:00Z".into(),
        state: WatermarkState::Partial,
        issue_codes: vec![WatermarkIssue::IncompleteTail],
    }
}
#[test]
fn snapshot_watermarks_survive_memory_and_file_restore_and_reject_missing_shape() {
    let root = tempfile::tempdir().unwrap();
    let rows = vec![watermark()];
    let facts = Collected {
        watermarks: rows.clone(),
        ..Default::default()
    };
    let snapshot = save_at(root.path(), facts.clone()).unwrap();
    let restored = load_at(
        root.path(),
        Some(&snapshot.manifest.snapshot_ref.snapshot_id),
    )
    .unwrap();
    assert_eq!(restored.manifest.watermarks, rows);
    let memory = memory(
        facts,
        "live:test".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(memory.manifest.watermarks, rows);
    let exported = save_at(root.path(), memory.live_collected().unwrap()).unwrap();
    assert_eq!(
        load_at(
            root.path(),
            Some(&exported.manifest.snapshot_ref.snapshot_id)
        )
        .unwrap()
        .manifest
        .watermarks,
        rows
    );
    let path = snapshot.directory.join("manifest.json");
    let mut raw: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    raw.as_object_mut().unwrap().remove("watermarks");
    fs::write(path, serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(
        load_at(
            root.path(),
            Some(&snapshot.manifest.snapshot_ref.snapshot_id)
        )
        .is_err()
    );
}

#[test]
fn future_watermark_header_precedes_future_shape_rejection() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = save_at(
        root.path(),
        Collected {
            watermarks: vec![watermark()],
            ..Default::default()
        },
    )
    .unwrap();
    let path = snapshot.directory.join("manifest.json");
    let mut raw: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    raw["watermarks"][0]["formatVersion"] = serde_json::json!(WATERMARK_FORMAT_VERSION + 1);
    raw["watermarks"][0]["futureField"] = serde_json::json!(true);
    fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let error = load_at(
        root.path(),
        Some(&snapshot.manifest.snapshot_ref.snapshot_id),
    )
    .err()
    .unwrap();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "UNSUPPORTED_VERSION"
    );
    raw["watermarks"][0]["formatVersion"] = serde_json::json!(WATERMARK_FORMAT_VERSION);
    fs::write(path, serde_json::to_vec(&raw).unwrap()).unwrap();
    let error = load_at(
        root.path(),
        Some(&snapshot.manifest.snapshot_ref.snapshot_id),
    )
    .err()
    .unwrap();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "SNAPSHOT_CORRUPT"
    );
}
