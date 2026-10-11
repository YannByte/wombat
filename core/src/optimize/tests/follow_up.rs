use super::*;
use crate::{
    adapters::{
        collect,
        contract::{DiscoveryRequest, RunContext},
    },
    config_dto::{UseBasisStatus, UseCoverage, UseSourceCompleteness, UseWindow},
    optimize::{detection::detect, follow_up::observe as observe_checked},
};
use serde_json::json;
use std::{fs, sync::Arc};

#[test]
fn follow_up_observes_only_canonical_associations_after_recheck_in_the_authorized_context() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    let root = dir.path().join("source");
    let other = dir.path().join("other");
    for p in [&a, &b, &root, &other] {
        fs::create_dir(p).unwrap();
    }
    let a = dunce::canonicalize(a).unwrap();
    let b = dunce::canonicalize(b).unwrap();
    let root = dunce::canonicalize(root).unwrap();
    let other = dunce::canonicalize(other).unwrap();
    let file = a.join("AGENTS.md");
    let log = |base: &Path, id: &str, project: &Path, times: &[(&str, Option<&str>)]| {
        fs::create_dir_all(base.join("sessions")).unwrap();
        let mut rows = vec![
            json!({"type":"session_meta","payload":{"id":id,"cwd":project}}),
            json!({"type":"turn_context","payload":{"turn_id":"u"}}),
        ];
        for (call, at) in times {
            rows.push(json!({"type":"response_item","timestamp":at,"payload":{"type":"function_call","call_id":call,"name":"read_file","arguments":json!({"path":file}).to_string()}}));
        }
        fs::write(
            base.join("sessions").join(format!("{id}.jsonl")),
            rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
        )
        .unwrap();
    };
    log(
        &root,
        "a",
        &a,
        &[
            ("old", Some("2026-09-30T23:59:59Z")),
            ("boundary", Some("2026-10-01T01:00:00+01:00")),
            ("new", Some("2026-10-01T01:00:00Z")),
            ("new", Some("2026-10-01T01:00:00Z")),
            ("fraction-later", Some("2026-10-01T02:00:00.500+01:00")),
            ("fraction-earlier", Some("2026-10-01T01:00:00.125Z")),
            ("future", Some("2026-10-03T00:00:00Z")),
            ("undated", None),
        ],
    );
    log(
        &root,
        "b",
        &b,
        &[("other-project", Some("2026-10-01T01:00:00Z"))],
    );
    log(
        &other,
        "other-source",
        &a,
        &[("other-source", Some("2026-10-01T01:00:00Z"))],
    );
    let data = collect(
        &DiscoveryRequest {
            roots: vec![root.clone(), other],
        },
        &RunContext::default(),
    );
    let source = data
        .sources
        .iter()
        .find(|s| s.source.root == root.to_string_lossy())
        .unwrap()
        .source
        .id
        .clone();
    let mut v = view();
    let mut s = detect(&v, &RuleParameters::default()).remove(0);
    v.items[0].kind = Kind::Rule;
    v.items[0].path = file.to_string_lossy().into();
    v.items[0].source_instance_id = source.clone();
    v.snapshot = Some(Arc::new(
        crate::usage_store::memory(
            data,
            "live:synthetic:followup".into(),
            crate::pricing_sync::current_at(dir.path()).unwrap(),
            None,
        )
        .unwrap(),
    ));
    v.checked = "2026-10-02T00:00:00Z".into();
    v.history_status = "current".into();
    s.item = v.items[0].clone();
    s.status = "verified".into();
    s.record_id = Some("record".into());
    s.checked_at = "2026-10-01T00:00:00Z".into();
    s.scope_project = Some(a.to_string_lossy().into());
    let result = observe(&[s.clone()], &v, None);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].status, FollowUpStatus::VersionUnknown);
    assert_eq!(result[0].observed_records, Some(3));
    let unknown = result[0].use_basis.as_ref().unwrap();
    assert_eq!(unknown.status, UseBasisStatus::Partial);
    assert_eq!(unknown.source_completeness, UseSourceCompleteness::Complete);
    assert_eq!(unknown.scope.project.as_deref(), Some(a.to_str().unwrap()));
    assert_eq!(unknown.scope.source_instance_ids, vec![source.clone()]);
    assert_eq!(
        unknown.coverage,
        UseCoverage {
            dispatch_gaps: Some(0),
            identity_gaps: Some(0),
            target_gaps: Some(0),
            time_gaps: Some(1),
            turn_gaps: Some(0),
        }
    );
    assert_eq!(
        result[0].last_record_at.as_deref(),
        Some("2026-10-01T01:00:00.500+00:00")
    );
    assert!(!result[0].absence_observable);
    assert_eq!(result[0].record_id, "record");
    s.scope_project = Some(b.to_string_lossy().into());
    let in_window = observe(&[s.clone()], &v, None);
    assert_eq!(in_window[0].observed_records, Some(1));
    assert_eq!(
        in_window[0].use_basis.as_ref().unwrap().status,
        UseBasisStatus::Observed
    );
    s.checked_at = "2026-10-01T02:00:00Z".into();
    let none = observe(&[s.clone()], &v, None);
    assert_eq!(none[0].status, FollowUpStatus::NoObservedRecords);
    // Project b has one timed canonical read at 01:00; after 02:00 its captured
    // window is reliably empty. Project a's undated read cannot taint this scope.
    assert_eq!(none[0].observed_records, Some(0));
    assert!(!none[0].absence_observable);
    let known_empty = none[0].use_basis.as_ref().unwrap();
    assert_eq!(known_empty.status, UseBasisStatus::Observed);
    assert_eq!(
        known_empty.source_completeness,
        UseSourceCompleteness::Complete
    );
    assert_eq!(
        known_empty.scope.project.as_deref(),
        Some(b.to_str().unwrap())
    );
    assert_eq!(known_empty.scope.source_instance_ids, vec![source]);
    assert_eq!(
        known_empty.scope.window,
        UseWindow::FollowUp {
            after: s.checked_at.clone(),
            through: v.checked.clone(),
        }
    );
    assert_eq!(
        known_empty.coverage,
        UseCoverage {
            dispatch_gaps: Some(0),
            identity_gaps: Some(0),
            target_gaps: Some(0),
            time_gaps: Some(0),
            turn_gaps: Some(0),
        }
    );
    let unavailable = observe(&[s.clone()], &v, Some("unrelated"));
    assert_eq!(unavailable[0].observed_records, None);
    let unavailable_basis = unavailable[0].use_basis.as_ref().unwrap();
    assert_eq!(unavailable_basis.status, UseBasisStatus::Unavailable);
    assert_eq!(
        unavailable_basis.source_completeness,
        UseSourceCompleteness::Unknown
    );
    assert_eq!(unavailable_basis.coverage.time_gaps, None);
    for state in ["updating", "unavailable"] {
        v.history_status = state.into();
        assert_eq!(
            observe(&[s.clone()], &v, None)[0].status,
            FollowUpStatus::Unavailable
        );
    }
    v.history_status = "current".into();
    v.items[0].kind = Kind::Hook;
    assert_eq!(
        observe(&[s.clone()], &v, None)[0].status,
        FollowUpStatus::Unavailable
    );
    s.status = "stillNeedsReview".into();
    assert!(observe(&[s], &v, None).is_empty());
}

#[test]
fn mcp_follow_up_requires_unambiguous_ownership_across_the_whole_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("sessions")).unwrap();
    let projects = [root.join("a"), root.join("b")];
    for (index, project) in projects.iter().enumerate() {
        fs::create_dir(project).unwrap();
        let call = json!({"type":"event_msg","timestamp":"2026-10-01T01:00:00Z","payload":{"type":"mcp_tool_call_end","call_id":"call","turn_id":"u","invocation":{"server":"shared","tool":"search"},"result":{"Ok":{"isError":true,"content":[]}}}});
        let rows = [
            json!({"type":"session_meta","payload":{"id":format!("t{index}"),"cwd":project}}),
            json!({"type":"turn_context","payload":{"turn_id":"u"}}),
            call.clone(),
            call,
        ];
        fs::write(
            source.join("sessions").join(format!("{index}.jsonl")),
            rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
        )
        .unwrap();
    }
    let data = collect(
        &DiscoveryRequest {
            roots: vec![source],
        },
        &RunContext::default(),
    );
    let source_id = data.sources[0].source.id.clone();
    let mut v = view();
    let mut s = detect(&v, &RuleParameters::default()).remove(0);
    v.items[0].kind = Kind::Mcp;
    v.items[0].native_key = Some("shared".into());
    v.items[0].source_instance_id = source_id;
    let mut local = v.items[0].clone();
    local.id = "another-physical-declaration".into();
    local.project = Some(projects[0].to_string_lossy().into());
    local.path = projects[0].join("config.toml").to_string_lossy().into();
    v.items.push(local);
    v.checked = "2026-10-02T00:00:00Z".into();
    v.history_status = "current".into();
    v.snapshot = Some(Arc::new(
        crate::usage_store::memory(
            data,
            "live:synthetic:mcp-followup".into(),
            crate::pricing_sync::current_at(dir.path()).unwrap(),
            None,
        )
        .unwrap(),
    ));
    s.item = v.items[0].clone();
    s.status = "verified".into();
    s.record_id = Some("global-record".into());
    s.checked_at = "2026-10-01T00:00:00Z".into();
    // The competing declaration has no handling record and is outside the requested page.
    s.scope_project = Some(projects[0].to_string_lossy().into());
    let ambiguous = observe(&[s.clone()], &v, None);
    assert_eq!(ambiguous[0].status, FollowUpStatus::NoObservedRecords);
    assert_eq!(ambiguous[0].observed_records, Some(0));
    assert_eq!(ambiguous[0].last_record_at, None);
    s.scope_project = Some(projects[1].to_string_lossy().into());
    let known = observe(&[s.clone()], &v, None);
    assert_eq!(known[0].status, FollowUpStatus::VersionUnknown);
    assert_eq!(known[0].observed_records, Some(1));
    s.scope_project = None;
    let unscoped = observe(&[s.clone()], &v, None);
    assert_eq!(unscoped[0].status, FollowUpStatus::VersionUnknown);
    assert_eq!(unscoped[0].observed_records, Some(1));
    s.scope_project = Some(projects[1].to_string_lossy().into());
    // Distinct historical cutoffs for one physical object do not create ownership ambiguity.
    let mut earlier = s.clone();
    earlier.record_id = Some("earlier-record".into());
    earlier.checked_at = "2026-09-30T00:00:00Z".into();
    assert!(
        observe(&[s, earlier], &v, None)
            .iter()
            .all(|o| o.observed_records == Some(1))
    );
}

fn observe(
    suggestions: &[Suggestion],
    view: &View,
    source: Option<&str>,
) -> Vec<crate::optimize_dto::FollowUpObservation> {
    observe_checked(suggestions, view, source).unwrap()
}
