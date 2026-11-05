use super::*;
use serde_json::json;
fn observe_args(args: serde_json::Value, cwd: &str, reads: &[&str]) -> OperationMatchObservation {
    let raw = serde_json::value::to_raw_value(&args).unwrap();
    let parsed = reads
        .iter()
        .map(|path| ParsedCommand::Read {
            path: Some((*path).to_owned()),
        })
        .collect::<Vec<_>>();
    observe(
        Some(&raw),
        Some("thread-owner"),
        Some(&CommandSource::Agent),
        Some(cwd),
        Some(&parsed),
    )
}
#[test]
fn exact_requests_keep_only_matching_digests_and_historical_targets() {
    let argv = json!(["/usr/bin/cat", "PRIVATE_TARGET"]);
    let a = observe_args(argv.clone(), "/historical", &["PRIVATE_TARGET"]);
    let b = observe_args(argv.clone(), "/historical", &["PRIVATE_TARGET"]);
    assert_eq!(a, b);
    assert_eq!(a.request_fingerprint.as_ref().unwrap().len(), 64);
    assert_eq!(a.receiver_owner.as_deref(), Some("thread-owner"));
    assert_eq!(a.read_targets[0].path, "/historical/PRIVATE_TARGET");
    assert_ne!(
        a.request_fingerprint,
        observe_args(json!(["/usr/bin/cat", "OTHER"]), "/historical", &[]).request_fingerprint
    );
    assert_ne!(
        a.request_fingerprint,
        observe_args(argv, "/other-cwd", &[]).request_fingerprint
    );
    let source = json!(["/usr/bin/compiler", "PRIVATE_SECRET_ARGUMENT"]);
    let value = observe_args(source, "/historical", &[]);
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(!encoded.contains("compiler"));
    assert!(!encoded.contains("PRIVATE_SECRET_ARGUMENT"));
    value.validate().unwrap();
}

#[test]
fn compound_and_multiline_request_equality_does_not_prove_reads_or_expected_failures() {
    for script in ["cat a; test -e b", "python3 - <<'PY'\nprint('x')\nPY"] {
        let argv = json!(["/bin/sh", "-lc", script]);
        let a = observe_args(argv.clone(), "/hist", &["a"]);
        assert_eq!(
            a.request_fingerprint,
            observe_args(argv, "/hist", &["a"]).request_fingerprint
        );
        assert!(a.request_fingerprint.is_some());
        assert!(a.read_targets.is_empty());
        assert!(!a.expected_nonzero);
        assert!(!serde_json::to_string(&a).unwrap().contains(script));
    }
    let invalid = observe_args(json!(["/bin/sh", "-c", "x\0y"]), "/hist", &[]);
    assert!(invalid.request_fingerprint.is_none());
}
#[test]
fn literal_shell_requests_verify_reads_without_storing_scripts() {
    let value = observe_args(
        json!(["/bin/zsh", "-lc", "cat 'one file'"]),
        "file:///historical/dir",
        &["one file"],
    );
    assert!(value.request_fingerprint.is_some());
    assert_eq!(value.read_targets[0].path, "/historical/dir/one file");
    assert!(!serde_json::to_string(&value).unwrap().contains("cat "));
    for script in [
        "cat $FILE",
        "cat *.rs",
        "cat a; cat b",
        "cat a | head",
        "cat $(name)",
        "cat `name`",
        "cat ~/a",
        "A=x cat a",
        "eval cat a",
        "source script",
        "cat a > out",
        "cat {a,b}",
    ] {
        let value = observe_args(json!(["/bin/sh", "-c", script]), "/historical", &["a"]);
        assert!(value.request_fingerprint.is_some(), "{script}");
        assert!(value.read_targets.is_empty());
        assert!(value.gaps.contains(&MatchGap::UnconfirmedRead));
    }
}
#[test]
fn interpreter_aliases_cannot_bypass_determinate_shell_matching() {
    for name in [
        "C:\\Windows\\System32\\CMD.EXE",
        "PowerShell.EXE",
        "PWSH.exe",
        "FISH.EXE",
        "ENV.exe",
        "EVAL",
        "BASH.EXE",
        "Sh.exe",
        "ZSH",
    ] {
        let value = observe_args(json!([name, "-c", "cat $FILE"]), "C:/hist", &["a"]);
        assert!(value.request_fingerprint.is_some(), "{name}");
        assert!(value.read_targets.is_empty(), "{name}");
    }
    for script in [
        "BASH.EXE -c cat",
        "CMD.EXE /c cat",
        "PowerShell.EXE cat",
        "if true",
        "for a in b",
    ] {
        let value = observe_args(json!(["sh", "-c", script]), "/hist", &[]);
        assert!(value.request_fingerprint.is_some(), "{script}");
    }
    let value = observe_args(json!(["BASH.EXE", "-c", "cat a"]), "C:/hist", &["a"]);
    assert!(value.request_fingerprint.is_some());
    assert_eq!(value.read_targets[0].path, "c:/hist/a");
}
#[test]
fn native_read_labels_need_verified_execution_targets_and_keep_one_target_once() {
    let value = observe_args(
        json!(["cat", "a", "b"]),
        "/historical",
        &["a", "a", "b", "wrong"],
    );
    assert_eq!(value.read_targets.len(), 2);
    assert!(value.gaps.contains(&MatchGap::UnconfirmedRead));
    for argv in [
        json!(["rg", "text", "a"]),
        json!(["ls", "a"]),
        json!(["cat", "-x", "a"]),
        json!(["cat", "-"]),
        json!(["sed", "-n", "/x/p", "a"]),
    ] {
        let value = observe_args(argv, "/historical", &["a"]);
        assert!(value.read_targets.is_empty());
    }
    for argv in [
        json!(["head", "-n", "20", "a"]),
        json!(["tail", "-n", "20", "a"]),
        json!(["sed", "-n", "1,20p", "a"]),
    ] {
        let value = observe_args(argv, "/historical", &["a"]);
        assert_eq!(value.read_targets[0].path, "/historical/a");
        // Read targets never invent requested/delivered ranges or historical content versions.
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("range"));
        assert!(!encoded.contains("content"));
    }
}
#[test]
fn targets_use_source_platform_and_historical_uri_without_host_filesystem() {
    assert_eq!(target("file:///hist", "a%20b").unwrap().path, "/hist/a%20b");
    assert_eq!(
        target("file:///hist", "file:///hist/a%20b").unwrap().path,
        "/hist/a b"
    );
    assert_eq!(
        target("C:\\hist", "folder\\a").unwrap().path,
        "c:/hist/folder/a"
    );
    assert_eq!(target("file:///C:/hist", "a").unwrap().path, "c:/hist/a");
    assert_eq!(
        target("\\\\server\\share\\hist", "a").unwrap().path,
        "//server/share/hist/a"
    );
    for (cwd, path) in [
        ("/hist", "../a"),
        ("/hist/link/..", "a"),
        ("relative", "a"),
        ("C:/hist", "/drive-root"),
        ("C:/hist", "C:relative"),
        ("/hist", "C:/foreign"),
        ("ssh://host/hist", "a"),
        ("/hist", "file://remote/a"),
        ("/hist", "file:///a%00b"),
    ] {
        assert!(target(cwd, path).is_none(), "{cwd} {path}");
    }
}
#[test]
fn missing_owner_and_user_shell_never_default_to_model_receiver() {
    let raw = serde_json::value::to_raw_value(&json!(["cat", "a"])).unwrap();
    for source in [
        None,
        Some(&CommandSource::Unknown),
        Some(&CommandSource::UserShell),
        Some(&CommandSource::UnifiedExecInteraction),
    ] {
        let value = observe(
            Some(&raw),
            Some("thread-owner"),
            source,
            Some("/hist"),
            None,
        );
        assert!(value.receiver_owner.is_none());
        assert!(value.gaps.contains(&MatchGap::MissingReceiver));
    }
    let value = observe(
        Some(&raw),
        None,
        Some(&CommandSource::Agent),
        Some("/hist"),
        None,
    );
    assert!(value.receiver_owner.is_none());
    let value = observe(
        Some(&raw),
        Some("owner"),
        Some(&CommandSource::Agent),
        None,
        None,
    );
    assert!(value.request_fingerprint.is_none());
    assert!(value.gaps.contains(&MatchGap::MissingHistoricalCwd));
}

#[test]
fn unified_startup_retains_only_an_explicit_locally_owned_receiver() {
    let raw = serde_json::value::to_raw_value(&json!(["cat", "a"])).unwrap();
    for owner in [Some("thread-owner"), None] {
        let value = observe(
            Some(&raw),
            owner,
            Some(&CommandSource::UnifiedExecStartup),
            Some("/hist"),
            None,
        );
        assert_eq!(value.receiver_owner.as_deref(), owner);
        assert_eq!(
            value.gaps.contains(&MatchGap::MissingReceiver),
            owner.is_none()
        );
    }
}
#[test]
fn no_match_search_commands_carry_expected_nonzero_evidence_separately() {
    for name in ["rg", "/usr/bin/grep", "diff", "cmp", "test"] {
        assert!(observe_args(json!([name, "a"]), "/hist", &[]).expected_nonzero);
    }
    for name in ["compiler", "cat"] {
        assert!(!observe_args(json!([name, "a"]), "/hist", &[]).expected_nonzero);
    }
}
#[test]
fn malformed_and_oversized_parameters_produce_no_prefix_digest_or_read_targets() {
    for args in [
        json!("cat a"),
        json!({"argv":["cat","a"]}),
        json!(["cat", 17]),
        json!(["cat", "a\0"]),
        json!([]),
    ] {
        let value = observe_args(args, "/hist", &["a"]);
        assert!(value.request_fingerprint.is_none());
        assert!(value.read_targets.is_empty());
        assert!(value.gaps.contains(&MatchGap::UnsupportedParameters));
    }
    for args in [
        json!(vec!["a"; 1025]),
        json!(["cat", "a".repeat(MATCH_STRING_BYTES + 1)]),
    ] {
        let value = observe_args(args, "/hist", &["a"]);
        assert!(value.request_fingerprint.is_none());
        assert!(value.read_targets.is_empty());
        assert!(value.gaps.contains(&MatchGap::ResourceLimit));
    }
}
#[test]
fn missing_completion_parameters_preserve_dispatch_facts_and_conflicts_latch_both_orders() {
    let a = observe_args(json!(["cat", "a"]), "/hist", &["a"]);
    let absent = observe(None, None, None, None, None);
    let mut combined = Some(a.clone());
    merge(&mut combined, &Some(absent));
    assert_eq!(
        combined.as_ref().unwrap().request_fingerprint,
        a.request_fingerprint
    );
    assert_eq!(combined.as_ref().unwrap().read_targets, a.read_targets);
    let b = observe_args(json!(["cat", "b"]), "/hist", &["b"]);
    for (first, second) in [(a.clone(), b.clone()), (b, a.clone())] {
        let mut combined = Some(first);
        merge(&mut combined, &Some(second));
        merge(&mut combined, &Some(a.clone()));
        let value = combined.unwrap();
        assert!(value.request_fingerprint.is_none());
        assert!(value.read_targets.is_empty());
        assert!(value.receiver_owner.is_none());
        assert!(value.gaps.contains(&MatchGap::ConflictingObservation));
    }
}
#[test]
fn matching_metadata_validates_current_version_and_bounded_digest_shapes() {
    let value = observe_args(json!(["cat", "a"]), "/hist", &["a"]);
    value.validate().unwrap();
    let mut invalid = value.clone();
    invalid.format_version = OPERATION_MATCH_VERSION + 1;
    assert!(invalid.validate().is_err());
    for digest in ["PRIVATE_RAW_COMMAND", &"A".repeat(64), &"0".repeat(65)] {
        let mut invalid = value.clone();
        invalid.request_fingerprint = Some(digest.into());
        assert!(invalid.validate().is_err());
    }
    let mut invalid = value.clone();
    invalid.receiver_owner = Some("\n".into());
    assert!(invalid.validate().is_err());
    let mut invalid = value;
    invalid.read_targets.push(invalid.read_targets[0].clone());
    assert!(invalid.validate().is_err());
}

#[test]
fn path_and_read_target_limits_preserve_independent_request_digest_without_a_target_prefix() {
    assert_eq!(target("C:/", ".").unwrap().path, "c:/");
    assert!(target("\\\\server", ".").is_none());
    assert_eq!(target("/hist", "///a").unwrap().path, "/a");
    let cwd = format!("/{}", "d".repeat(MATCH_STRING_BYTES));
    let value = observe_args(json!(["cat", "a", "b"]), &cwd, &["a", "b"]);
    assert!(value.request_fingerprint.is_some());
    assert!(value.read_targets.is_empty());
    assert!(value.gaps.contains(&MatchGap::ResourceLimit));
    value.validate().unwrap();
    let mut invalid = observe_args(json!(["cat", "a"]), "/hist", &["a"]);
    invalid.read_targets[0].path = "relative".into();
    assert!(invalid.validate().is_err());
    invalid.read_targets[0].path = "/hist/../a".into();
    assert!(invalid.validate().is_err());
    invalid.read_targets[0].platform = SourcePathPlatform::Windows;
    for path in ["//server", "//", "c:/hist\\..\\a"] {
        invalid.read_targets[0].path = path.into();
        assert!(invalid.validate().is_err(), "{path}");
    }
}

#[test]
fn native_shape_rejects_unobserved_execution_parameters_without_allocating_output_bodies() {
    let native = json!({"type":"commandExecution","id":"x","command":["cat","a"],"cwd":"/hist","source":"agent","status":"completed","stdout":"PRIVATE_OUTPUT","interaction_input":null});
    let check = |value: &serde_json::Value| {
        let raw = serde_json::value::to_raw_value(value).unwrap();
        command_shape(Some(&raw))
    };
    assert!(check(&native));
    for key in [
        "env",
        "stdin",
        "future_execution_parameter",
        "plugin_id",
        "script_path",
        "interaction_input",
    ] {
        let mut invalid = native.clone();
        invalid[key] = json!("PRIVATE_PARAMETER");
        assert!(!check(&invalid), "{key}");
    }
    assert!(!command_shape(None));
}
