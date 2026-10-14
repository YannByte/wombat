use super::*;
fn row() -> SourceWatermark {
    SourceWatermark {
        original_source: None,
        format_version: crate::adapters::contract::WATERMARK_FORMAT_VERSION,
        source_instance_id: "s".into(),
        file_id: "f".into(),
        generation: None,
        committed_offset: 0,
        observed_bytes: Some(0),
        observed_at: "2026-10-05T00:00:00Z".into(),
        state: WatermarkState::Complete,
        issue_codes: vec![],
    }
}
#[test]
fn empty_generation_and_unknown_headers_are_not_complete_coverage() {
    assert!(row().validate().is_ok());
    let mut value = row();
    value.generation = Some(String::new());
    assert!(value.validate().is_err());
    value = row();
    value.format_version = WATERMARK_FORMAT_VERSION + 1;
    assert!(value.validate().is_err());
    value = row();
    value.observed_bytes = None;
    assert!(value.validate().is_err());
    let mut encoded = serde_json::to_value(row()).unwrap();
    encoded.as_object_mut().unwrap().remove("formatVersion");
    assert!(serde_json::from_value::<SourceWatermark>(encoded).is_err());
}
#[test]
fn unavailability_preserves_committed_identity_and_offset() {
    let mut value = row();
    value.generation = Some("g".into());
    value.committed_offset = 20;
    value.observed_bytes = Some(20);
    let missing = value.unavailable(
        WatermarkState::Missing,
        WatermarkIssue::SourceMissing,
        "2026-10-05T01:00:00Z",
    );
    assert_eq!(missing.committed_offset, 20);
    assert_eq!(missing.generation, value.generation);
    assert_eq!(missing.observed_bytes, None);
    assert!(missing.validate().is_ok());
    assert!(validate_watermarks(&[value.clone(), value]).is_err());
}
