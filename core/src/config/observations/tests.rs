//! Synthetic independent versions and pinned query windows, without source reads.
use super::*;
fn view() -> View {
    View {
        snapshot: None,
        items: vec![],
        issues: vec![],
        projects: vec![],
        roots: vec![],
        project_roots: vec![],
        revision: String::new(),
        checked: "2026-10-01T23:30:00Z".into(),
        history_status: "unavailable".into(),
        analysis: Default::default(),
        hook_registry: Default::default(),
        observation_versions: Default::default(),
        config_collection: Default::default(),
    }
}
fn item() -> Item {
    Item {
        id: "synthetic-item".into(),
        name: "synthetic".into(),
        kind: Kind::Skill,
        source_instance_id: "synthetic-source".into(),
        path: "/synthetic/SKILL.md".into(),
        project: None,
        native_key: None,
        authorized_projects: vec![],
        source_contexts: vec![],
        configured_state: "discovered".into(),
        content_hash: "synthetic-content".into(),
        observed_at: "2026-10-01T23:30:00Z".into(),
        current: true,
        stale: false,
        bytes: Some(10),
        content_tokens: Some(5),
        estimate_status: "estimated".into(),
        characters: Some(10),
        measurement_status: "complete".into(),
        bytes_source: Some("completeUtf8File".into()),
        estimate: Some(ContentEstimate {
            tokens: 5,
            encoding: "synthetic-encoding".into(),
            method: "synthetic-method-v1".into(),
            payload: "completeUtf8File".into(),
            content_hash: "synthetic-content".into(),
            applicability: "referenceEncodingOnly".into(),
            tokenizer_version: Some("synthetic-tokenizer-v1".into()),
        }),
        skill_metadata: None,
        body_token_estimate: None,
        body_estimate_status: "unknown".into(),
        context_footprint: None,
        usage_count: None,
        use_basis: None,
        last_record_at: None,
        observation: Observation::Unknown,
        counts: Counts::default(),
        related_turns: 0,
        related_tasks: 0,
        usage: None,
    }
}
fn populated() -> View {
    let mut view = view();
    view.items.push(item());
    view
}

#[test]
fn independent_versions_ignore_observation_times_and_historical_counters() {
    let first = populated();
    let mut next = populated();
    next.checked = "2026-10-02T23:30:00Z".into();
    next.items[0].observed_at = next.checked.clone();
    next.items[0].usage_count = Some(999);
    next.items[0].related_turns = 999;
    next.items[0].last_record_at = Some(next.checked.clone());
    next.hook_registry.checked_at = Some(next.checked.clone());
    let first = first.observation_versions().unwrap();
    let next = next.observation_versions().unwrap();
    assert_eq!(first.format_version, 1);
    assert_eq!(first.config_revision, next.config_revision);
    assert_eq!(first.analysis_revision, next.analysis_revision);
    assert_eq!(first.host_revision, next.host_revision);
    assert_ne!(first.cutoff, next.cutoff);
    assert_eq!(
        first.combined_revision().unwrap(),
        next.combined_revision().unwrap()
    );
}
#[test]
fn measurements_methods_statuses_and_memberships_change_only_configuration_version() {
    let first = populated();
    let baseline = first.observation_versions().unwrap();
    for kind in 0..6 {
        let mut changed = populated();
        match kind {
            0 => changed.items[0].estimate.as_mut().unwrap().method = "synthetic-method-v2".into(),
            1 => changed.items[0].measurement_status = "partial".into(),
            2 => changed.items[0]
                .authorized_projects
                .push("/synthetic/project".into()),
            3 => changed.items[0].body_token_estimate = changed.items[0].estimate.clone(),
            4 => changed.items[0].source_contexts.push(SourceContext {
                inventory_id: "synthetic-inventory".into(),
                global: true,
                source_instance_id: "other-source".into(),
                content_hash: "synthetic-content".into(),
                configured_state: "discovered".into(),
                counts: Counts::default(),
                observation: Observation::Unknown,
                last_record_at: None,
            }),
            5 => changed.projects.push("/synthetic/authorized".into()),
            _ => unreachable!(),
        }
        let next = changed.observation_versions().unwrap();
        assert_ne!(baseline.config_revision, next.config_revision);
        assert_eq!(baseline.analysis_revision, next.analysis_revision);
        if kind == 5 {
            assert_ne!(baseline.host_revision, next.host_revision);
        } else {
            assert_eq!(baseline.host_revision, next.host_revision);
        }
    }
}
#[test]
fn host_status_and_native_observation_have_an_independent_content_revision() {
    let first = populated();
    let baseline = first.observation_versions().unwrap();
    for kind in 0..3 {
        let mut changed = populated();
        match kind {
            0 => changed.hook_registry.native_version = Some("synthetic-native-version".into()),
            1 => changed.hook_registry.status = HookRegistryStatus::Partial,
            2 => changed.hook_registry.contexts.push(HookContext {
                project: "/synthetic/project".into(),
                complete: false,
                registrations: vec![],
            }),
            _ => unreachable!(),
        }
        let next = changed.observation_versions().unwrap();
        assert_ne!(baseline.host_revision, next.host_revision);
        assert_eq!(baseline.config_revision, next.config_revision);
        assert_eq!(baseline.analysis_revision, next.analysis_revision);
    }
}
#[test]
fn safe_facts_are_hashed_once_and_set_order_is_deterministic() {
    let mut first = populated();
    first.items[0].authorized_projects = vec!["/b".into(), "/a".into()];
    first.projects = vec!["/b".into(), "/a".into()];
    let mut second = populated();
    second.items[0].authorized_projects = vec!["/a".into(), "/b".into(), "/a".into()];
    second.projects = vec!["/a".into(), "/b".into()];
    let left = first.observation_versions().unwrap();
    assert!(std::ptr::eq(left, first.observation_versions().unwrap()));
    assert_eq!(left, second.observation_versions().unwrap());
}
#[test]
fn missing_body_path_never_triggers_a_query_read_and_snapshot_identity_is_independent() {
    let root = tempfile::tempdir().unwrap();
    let mut first = populated();
    let mut second = populated();
    for (view, id) in [
        (&mut first, "live:synthetic-a"),
        (&mut second, "live:synthetic-b"),
    ] {
        let prices = crate::pricing_sync::current_at(root.path()).unwrap();
        view.snapshot = Some(Arc::new(
            crate::usage_store::memory(Default::default(), id.into(), prices, None).unwrap(),
        ));
    }
    assert_eq!(
        first.observation_versions().unwrap(),
        second.observation_versions().unwrap()
    );
}
#[test]
fn invalid_cutoff_is_visible_and_cannot_initialize_a_success() {
    let mut view = populated();
    view.checked = "invalid".into();
    assert!(
        view.observation_versions()
            .unwrap_err()
            .to_string()
            .contains("截止时间无效")
    );
    assert!(view.observation_versions.get().is_none());
    view.checked = "2026-10-02T07:30:00+08:00".into();
    assert_eq!(
        view.observation_versions().unwrap().cutoff,
        "2026-10-01T23:30:00.000000000Z"
    );
}
#[test]
fn default_and_partially_explicit_windows_follow_the_fixed_view_clock() {
    let checked = "2026-09-30T23:30:00Z";
    let scope = Scope {
        timezone: Some("Asia/Shanghai".into()),
        ..Default::default()
    };
    let (normalized, _) = normalize_at(&scope, checked).unwrap();
    assert_eq!(normalized.since.as_deref(), Some("2026-09-02"));
    assert_eq!(normalized.until.as_deref(), Some("2026-10-02"));
    let (same, _) = normalize_at(&scope, "2026-10-01T00:30:00+01:00").unwrap();
    assert_eq!(
        serde_json::to_value(normalized).unwrap(),
        serde_json::to_value(same).unwrap()
    );
    let request = Request {
        scope: Scope {
            until: Some("2020-01-02".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    // The explicit old endpoint is valid before selection; request validation has no today.
    validate(&request).unwrap();
    let (old, _) = normalize_at(&request.scope, "2020-01-01T23:30:00Z").unwrap();
    assert_eq!(old.since.as_deref(), Some("2019-12-03"));
    assert_eq!(old.until.as_deref(), Some("2020-01-02"));
    assert!(normalize_at(&request.scope, "2026-10-01T00:00:00Z").is_err());
}
#[test]
fn pinned_query_keeps_its_default_window_and_rejects_invalid_checked_time() {
    let mut view = view();
    view.checked = "2020-01-01T23:30:00Z".into();
    let request = Request {
        scope: Scope {
            until: Some("2020-01-02".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = execute(request, "config:synthetic".into(), &view).unwrap();
    assert_eq!(result.scope.since.as_deref(), Some("2019-12-03"));
    assert_eq!(result.scope.until.as_deref(), Some("2020-01-02"));
    view.checked = "invalid".into();
    assert!(execute(Request::default(), "config:synthetic".into(), &view).is_err());
    assert!(
        normalize_at(
            &Scope {
                all_time: Some(true),
                ..Default::default()
            },
            "invalid"
        )
        .is_err()
    );
}
#[test]
fn explicit_date_and_timezone_validation_remain_fail_closed() {
    for scope in [
        Scope {
            since: Some("invalid".into()),
            ..Default::default()
        },
        Scope {
            since: Some("2026-10-02".into()),
            until: Some("2026-10-01".into()),
            ..Default::default()
        },
        Scope {
            all_time: Some(true),
            until: Some("2026-10-01".into()),
            ..Default::default()
        },
        Scope {
            timezone: Some("invalid".into()),
            ..Default::default()
        },
    ] {
        assert!(
            validate(&Request {
                scope,
                ..Default::default()
            })
            .is_err()
        );
    }
}

#[test]
fn empty_complete_failed_and_resource_limited_collections_have_distinct_revisions() {
    let baseline = view();
    let baseline = baseline.observation_versions().unwrap();
    let mut revisions = BTreeSet::from([baseline.config_revision.clone()]);
    for code in [
        "configUnreadable",
        "resourceLimited",
        "authorizationUnavailable",
    ] {
        let mut changed = view();
        changed.config_collection = ConfigCollection::capture(
            &[Issue {
                code: code.into(),
                path: Some("/synthetic/config".into()),
            }],
            vec![],
        );
        let next = changed.observation_versions().unwrap();
        assert!(revisions.insert(next.config_revision.clone()));
        assert_eq!(baseline.analysis_revision, next.analysis_revision);
        assert_eq!(baseline.host_revision, next.host_revision);
    }
    let mut missing = populated();
    missing.items[0].measurement_status = "missing".into();
    let complete = populated();
    assert_ne!(
        missing.observation_versions().unwrap().config_revision,
        complete.observation_versions().unwrap().config_revision
    );
    let states = serde_json::to_value(missing.config_collection.fact(&missing.items)).unwrap();
    assert_eq!(states[0], serde_json::json!([false, true, false, false]));
}

#[test]
fn collector_issues_are_canonical_and_do_not_mix_analysis_host_or_history() {
    let mut first = view();
    first.config_collection = ConfigCollection::capture(
        &[
            Issue {
                code: "configUnreadable".into(),
                path: Some("/b".into()),
            },
            Issue {
                code: "resourceLimited".into(),
                path: Some("/a".into()),
            },
        ],
        vec![],
    );
    let mut second = view();
    second.config_collection = ConfigCollection::capture(
        &[
            Issue {
                code: "resourceLimited".into(),
                path: Some("/a".into()),
            },
            Issue {
                code: "configUnreadable".into(),
                path: Some("/b".into()),
            },
            Issue {
                code: "effectiveConfigUnknown".into(),
                path: None,
            },
            Issue {
                code: "hookEffectiveRegistryUnavailable".into(),
                path: None,
            },
        ],
        vec![],
    );
    second.history_status = "complete".into();
    second.issues.push(Issue {
        code: "inventoryCacheUnavailable".into(),
        path: None,
    });
    second.analysis.issues.push(Issue {
        code: "staticAnalysisResourceLimited".into(),
        path: None,
    });
    let first = first.observation_versions().unwrap();
    let second = second.observation_versions().unwrap();
    assert_eq!(first.config_revision, second.config_revision);
    assert_eq!(first.host_revision, second.host_revision);
    assert_ne!(first.analysis_revision, second.analysis_revision);
}

fn registration(id: &str) -> HookRegistration {
    HookRegistration {
        item_id: id.into(),
        native_key: id.into(),
        content_hash: "safe-content".into(),
        registration_hash: "safe-registration".into(),
        enabled: true,
        trust: HookTrust::Trusted,
        handler: HookHandler::Command,
        source: "synthetic".into(),
        plugin_id: None,
    }
}
#[test]
fn host_registry_order_is_canonical_but_full_registration_facts_remain_bound() {
    let mut first = view();
    first.hook_registry.contexts = vec![
        HookContext {
            project: "/b".into(),
            complete: true,
            registrations: vec![registration("2"), registration("1")],
        },
        HookContext {
            project: "/a".into(),
            complete: false,
            registrations: vec![],
        },
    ];
    let mut second = view();
    second.hook_registry = first.hook_registry.clone();
    second.hook_registry.contexts.reverse();
    second.hook_registry.contexts[1].registrations.reverse();
    assert_eq!(
        first.observation_versions().unwrap(),
        second.observation_versions().unwrap()
    );
    let mut changed = view();
    changed.hook_registry = first.hook_registry.clone();
    changed.hook_registry.contexts[0].registrations[0].trust = HookTrust::Modified;
    assert_ne!(
        first.observation_versions().unwrap().host_revision,
        changed.observation_versions().unwrap().host_revision
    );
}

#[test]
fn host_revision_binds_effective_and_requested_authorization() {
    let baseline = view();
    let baseline = baseline.observation_versions().unwrap();
    for scope in 0..4 {
        let mut next = view();
        match scope {
            0 => next.roots.push("/requested/source".into()),
            1 => next
                .config_collection
                .source_roots
                .push("/effective/source".into()),
            2 => next.project_roots.push("/requested/project".into()),
            3 => next.projects.push("/effective/project".into()),
            _ => unreachable!(),
        }
        let next = next.observation_versions().unwrap();
        assert_ne!(baseline.host_revision, next.host_revision);
        assert_ne!(baseline.config_revision, next.config_revision);
        assert_eq!(baseline.analysis_revision, next.analysis_revision);
    }
}
