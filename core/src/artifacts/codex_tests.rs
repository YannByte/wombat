use super::*;
use std::{cell::Cell, fs, time::SystemTime};
struct Host {
    paths: codex::Paths,
    process: Cell<UseState>,
    usage: Cell<UseState>,
    installed: Cell<Option<u64>>,
    staged: Cell<Option<u64>>,
}
impl projects::Host for Host {
    fn git(&self, _: &Path, _: &scan::Budget<'_>) -> projects::Git {
        unreachable!("Codex checks do not read a repository")
    }
    fn usage(&self, _: &Path, _: &scan::Budget<'_>) -> UseState {
        let s = self.usage.replace(UseState::Unknown);
        self.usage.set(s.clone());
        s
    }
    fn codex_paths(&self) -> codex::Paths {
        self.paths.clone()
    }
    fn process(&self, _: bool, _: bool, _: &scan::Budget<'_>) -> UseState {
        let s = self.process.replace(UseState::Unknown);
        self.process.set(s.clone());
        s
    }
    fn installed_build(&self, _: &codex::Paths, _: &scan::Budget<'_>) -> Option<u64> {
        self.installed.get()
    }
    fn bundle_build(&self, _: &Path, _: &scan::Budget<'_>) -> Option<u64> {
        self.staged.get()
    }
}
fn write(p: &Path, bytes: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}
fn old(p: &Path) {
    fs::File::open(p)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(31 * 86400))
        .unwrap();
}
fn fixture() -> (tempfile::TempDir, Host, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let home = dunce::canonicalize(d.path()).unwrap();
    let project = home.join("project");
    fs::create_dir(&project).unwrap();
    let h = Host {
        paths: codex::Paths {
            codex: home.join("codex"),
            applications: vec![],
            home,
            supported: true,
        },
        process: Cell::new(UseState::NoObservedOpenFiles),
        usage: Cell::new(UseState::NoObservedOpenFiles),
        installed: Cell::new(Some(100)),
        staged: Cell::new(Some(90)),
    };
    (d, h, project)
}
fn inspect(project: &Path, h: &Host) -> Response {
    execute_with(
        Request::Inspect {
            project_root: project.to_str().unwrap().into(),
            paths: vec![],
            keep_paths: vec![],
            product_temporary: false,
            codex: true,
        },
        &|| false,
        h,
    )
    .unwrap()
}
fn apply(r: &Response, ids: Vec<String>, h: &Host) -> Result<Response> {
    execute_with(
        Request::Apply {
            baseline: Box::new(r.clone()),
            selection: ids,
        },
        &|| false,
        h,
    )
}
#[cfg(unix)]
fn daemon(h: &Host) -> PathBuf {
    let root = h.paths.codex.join("packages/app-server-daemon");
    for v in [
        "1.0.0-darwin-arm64",
        "2.0.0-darwin-arm64",
        "3.0.0-darwin-arm64",
    ] {
        write(
            &root.join("releases").join(v).join("bin"),
            b"synthetic runtime",
        );
    }
    write(&root.join("install.lock"), b"");
    write(&root.join("auto-update-version"), b"3.0.0-darwin-arm64");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("releases/2.0.0-darwin-arm64"),
        root.join("current"),
    )
    .unwrap();
    root
}
#[cfg(unix)]
#[test]
fn codex_six_rules_keep_state_active_versions_pending_updates_and_recent_reports() {
    let (_d, h, p) = fixture();
    let root = daemon(&h);

    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("item"), b"cache");
    let stage = h
        .paths
        .home
        .join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation/entry");
    write(&stage.join("Codex.app/Contents/Info.plist"), b"synthetic");
    let marketplace = h
        .paths
        .codex
        .join(".tmp/marketplaces/.staging/marketplace-add-old");
    write(&marketplace.join("SKILL.md"), b"downloaded bundle");
    old(&marketplace);
    let runtime = h.paths.home.join(".cache/codex-runtimes/temp-failed");
    write(&runtime.join("partial"), b"incomplete");
    let active = h.paths.home.join(".cache/codex-runtimes/live");
    write(&active.join("runtime.json"), b"{}");
    fs::create_dir_all(active.join("dependencies/node")).unwrap();
    let pending = h
        .paths
        .home
        .join("Library/Application Support/Codex/Crashpad/pending");
    write(&pending.join("old.dmp"), b"old report");
    old(&pending.join("old.dmp"));
    write(&pending.join("new.dmp"), b"recent report");
    write(&h.paths.codex.join("sessions/keep.jsonl"), b"conversation");
    write(&h.paths.codex.join("state_5.sqlite"), b"state");
    let r = inspect(&p, &h);
    assert!(!r.partial);
    let candidates: Vec<_> = r
        .items
        .iter()
        .filter(|i| i.assessment == Assessment::Candidate)
        .collect();
    assert_eq!(candidates.len(), 6);
    assert!(
        r.items
            .iter()
            .any(|i| i.path == active.to_str().unwrap() && i.assessment == Assessment::Protected)
    );
    let ids = candidates.iter().map(|i| i.id.clone()).collect();
    let removed = apply(&r, ids, &h).unwrap();
    assert!(!removed.partial);
    assert!(
        removed
            .items
            .iter()
            .all(|i| matches!(i.removal, Some(Removal::Removed)))
    );
    assert!(root.join("releases/2.0.0-darwin-arm64/bin").is_file());
    assert!(root.join("releases/3.0.0-darwin-arm64/bin").is_file());
    assert!(active.join("runtime.json").is_file());
    assert!(pending.join("new.dmp").is_file());
    assert!(h.paths.codex.join("sessions/keep.jsonl").is_file());
    assert!(h.paths.codex.join("state_5.sqlite").is_file());
}
#[cfg(unix)]
#[test]
fn daemon_pointer_and_install_lock_changes_block_the_original_selection() {
    let (_d, h, p) = fixture();
    let root = daemon(&h);

    let before = inspect(&p, &h);
    let target = before
        .items
        .iter()
        .find(|i| i.assessment == Assessment::Candidate)
        .unwrap();
    let lock = codex::lock(&h.paths).unwrap();
    assert!(apply(&before, vec![target.id.clone()], &h).is_err());
    drop(lock);
    fs::remove_file(root.join("current")).unwrap();
    std::os::unix::fs::symlink(
        root.join("releases/1.0.0-darwin-arm64"),
        root.join("current"),
    )
    .unwrap();
    let after = apply(&before, vec![target.id.clone()], &h).unwrap();
    assert!(after.partial);
    assert!(matches!(after.items[0].removal, Some(Removal::Skipped)));
    assert!(Path::new(&target.path).is_dir());
}
#[test]
fn process_restart_and_new_runtime_completion_stop_cleanup() {
    let (_d, h, p) = fixture();
    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("item"), b"cache");

    let r = inspect(&p, &h);
    let id = r.items[0].id.clone();
    h.process.set(UseState::InUse);
    let result = apply(&r, vec![id], &h).unwrap();
    assert!(result.partial);
    assert!(cache.join("item").is_file());
    h.process.set(UseState::NoObservedOpenFiles);
    let runtime = h.paths.home.join(".cache/codex-runtimes/partial-1");
    write(&runtime.join("item"), b"partial");
    let r = inspect(&p, &h);
    let id = r
        .items
        .iter()
        .find(|i| i.kind == Kind::CodexRuntime)
        .unwrap()
        .id
        .clone();
    write(&runtime.join("runtime.json"), b"{}");
    fs::create_dir_all(runtime.join("dependencies/python")).unwrap();
    let after = apply(&r, vec![id], &h).unwrap();
    assert!(after.partial);
    assert!(runtime.is_dir());
}
#[test]
fn unknown_process_open_files_future_updates_and_unrecognized_state_are_preserved() {
    let (_d, h, p) = fixture();
    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("item"), b"cache");

    h.process.set(UseState::Unknown);
    assert_eq!(inspect(&p, &h).items[0].assessment, Assessment::Unknown);
    h.process.set(UseState::NoObservedOpenFiles);
    h.usage.set(UseState::InUse);
    assert_eq!(inspect(&p, &h).items[0].assessment, Assessment::Protected);
    h.usage.set(UseState::NoObservedOpenFiles);
    let stage = h
        .paths
        .home
        .join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation/future");
    write(&stage.join("Codex.app/Contents/Info.plist"), b"bundle");
    old(&stage);
    h.staged.set(Some(200));
    let r = inspect(&p, &h);
    let future = r
        .items
        .iter()
        .find(|i| i.kind == Kind::CodexUpdateStage)
        .unwrap();
    assert_eq!(future.assessment, Assessment::Protected);
    assert!(future.reasons.contains(&Reason::PendingUpdate));
    assert!(apply(&r, vec![future.id.clone()], &h).is_err());
    assert!(apply(&r, vec![], &h).is_err());
    let secret = h.paths.codex.join("state_5.sqlite");
    write(&secret, b"keep");
    let r = execute_with(
        Request::Inspect {
            project_root: p.to_str().unwrap().into(),
            paths: vec![secret.to_str().unwrap().into()],
            keep_paths: vec![],
            product_temporary: false,
            codex: false,
        },
        &|| false,
        &h,
    )
    .unwrap();
    assert_eq!(r.items[0].assessment, Assessment::Protected);
}
#[cfg(unix)]
#[test]
fn linked_roots_are_kept_and_packaged_links_never_delete_their_targets() {
    let (_d, h, p) = fixture();
    let outside = h.paths.home.join("outside");
    write(&outside.join("keep"), b"keep");
    let root = h.paths.home.join("Library/Caches/Codex/Default");
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("Cache")).unwrap();

    let r = inspect(&p, &h);
    assert_eq!(r.items[0].assessment, Assessment::Protected);
    fs::remove_file(root.join("Cache")).unwrap();
    write(&root.join("Cache/file"), b"cache");
    std::os::unix::fs::symlink(&outside, root.join("Cache/link")).unwrap();
    let r = inspect(&p, &h);
    assert_eq!(r.items[0].assessment, Assessment::Candidate);
    apply(&r, vec![r.items[0].id.clone()], &h).unwrap();
    assert!(outside.join("keep").is_file());
}

#[cfg(unix)]
#[test]
fn broken_current_invalid_marker_and_directory_lock_protocol_never_produce_candidates() {
    let (_d, h, p) = fixture();
    let root = daemon(&h);

    fs::remove_file(root.join("current")).unwrap();
    let r = inspect(&p, &h);
    assert!(
        !r.items
            .iter()
            .any(|i| i.assessment == Assessment::Candidate)
    );
    assert!(r.partial);
    std::os::unix::fs::symlink(
        root.join("releases/2.0.0-darwin-arm64"),
        root.join("current"),
    )
    .unwrap();
    write(&root.join("auto-update-version"), b"../outside");
    let r = inspect(&p, &h);
    assert!(
        r.items
            .iter()
            .all(|i| i.assessment != Assessment::Candidate)
    );
    write(&root.join("auto-update-version"), b"3.0.0-darwin-arm64");
    fs::create_dir(root.join("install.lock.d")).unwrap();
    let r = inspect(&p, &h);
    assert!(r.partial);
    assert!(
        r.items
            .iter()
            .all(|i| i.assessment != Assessment::Candidate)
    );
}
#[test]
fn explicit_keep_unknown_runtime_and_crash_subdirectories_survive() {
    let (_d, h, p) = fixture();
    let root = h.paths.home.join(".cache/codex-runtimes/unrecognized");
    write(&root.join("dependencies/other"), b"unknown dependency");
    let pending = h
        .paths
        .home
        .join("Library/Application Support/Codex/Crashpad/pending");
    write(&pending.join("nested/item.dmp"), b"keep");
    old(&pending.join("nested"));
    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("file"), b"cache");

    let r = execute_with(
        Request::Inspect {
            project_root: p.to_str().unwrap().into(),
            paths: vec![],
            keep_paths: vec![cache.to_str().unwrap().into()],
            product_temporary: false,
            codex: true,
        },
        &|| false,
        &h,
    )
    .unwrap();
    assert!(
        r.items
            .iter()
            .all(|i| i.assessment != Assessment::Candidate)
    );
    assert!(
        r.items
            .iter()
            .any(|i| i.path == root.to_str().unwrap() && i.assessment == Assessment::Review)
    );
    assert!(
        r.items
            .iter()
            .any(|i| i.path == cache.to_str().unwrap() && i.reasons.contains(&Reason::UserKept))
    );
    assert!(pending.join("nested/item.dmp").is_file());
}
#[test]
fn already_absent_is_distinct_from_removed_and_cancelled_apply_changes_nothing() {
    let (_d, h, p) = fixture();
    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("file"), b"cache");

    let r = inspect(&p, &h);
    let id = r.items[0].id.clone();
    assert!(
        execute_with(
            Request::Apply {
                baseline: Box::new(r.clone()),
                selection: vec![id.clone()]
            },
            &|| true,
            &h
        )
        .is_err()
    );
    assert!(cache.join("file").is_file());
    fs::remove_dir_all(&cache).unwrap();
    let result = apply(&r, vec![id], &h).unwrap();
    assert!(matches!(
        result.items[0].removal,
        Some(Removal::AlreadyAbsent)
    ));
    assert!(!result.partial);
}
#[test]
fn unknown_installed_build_uses_age_only_without_inventing_a_version() {
    let (_d, h, p) = fixture();
    let stage = h
        .paths
        .home
        .join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation/stale");
    write(&stage.join("Codex.app/Contents/Info.plist"), b"metadata");
    old(&stage);
    h.installed.set(None);

    let r = inspect(&p, &h);
    let item = r
        .items
        .iter()
        .find(|i| i.kind == Kind::CodexUpdateStage)
        .unwrap();
    assert_eq!(item.assessment, Assessment::Candidate);
    let evidence = item.codex_evidence.as_ref().unwrap();
    assert_eq!(evidence.installed_build, None);
    assert_eq!(evidence.minimum_age_days, Some(30));
}
#[test]
fn an_incomplete_staged_inventory_cannot_be_applied_even_with_other_known_candidates() {
    let (_d, h, p) = fixture();
    let stage = h
        .paths
        .home
        .join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation/many");
    for i in 0..1002 {
        write(
            &stage.join(format!("app{i:04}.app/Contents/Info.plist")),
            b"metadata",
        );
    }

    let r = inspect(&p, &h);
    let item = r
        .items
        .iter()
        .find(|i| i.kind == Kind::CodexUpdateStage)
        .unwrap();
    assert_eq!(item.assessment, Assessment::Unknown);
    assert!(!item.observation.as_ref().unwrap().complete);
    assert!(apply(&r, vec![item.id.clone()], &h).is_err());
    assert!(stage.is_dir());
}
#[cfg(target_os = "macos")]
#[test]
fn native_plist_parsing_accepts_decimal_builds_and_rejects_foreign_bundles() {
    let (_d, h, _p) = fixture();
    let app = h.paths.home.join("Codex.app");
    let plist = app.join("Contents/Info.plist");
    let xml = |id: &str, version: &str| {
        format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleVersion</key><string>{version}</string></dict></plist>"
        )
    };
    write(&plist, xml("com.openai.codex", "0123").as_bytes());
    assert_eq!(
        codex::bundle_build(&app, &scan::Budget::new(&|| false)),
        Some(123)
    );
    write(&plist, xml("foreign.app", "123").as_bytes());
    assert_eq!(
        codex::bundle_build(&app, &scan::Budget::new(&|| false)),
        None
    );
    write(&plist, xml("com.openai.codex", "invalid").as_bytes());
    assert_eq!(
        codex::bundle_build(&app, &scan::Budget::new(&|| false)),
        None
    );
}

#[test]
fn old_crash_reports_are_streamed_in_bounded_batches_without_touching_recent_files() {
    let (_d, h, p) = fixture();
    let pending = h
        .paths
        .home
        .join("Library/Application Support/Codex/Crashpad/pending");
    for i in 0..105 {
        let path = pending.join(format!("old-{i:04}.dmp"));
        write(&path, b"report");
        old(&path);
    }
    write(&pending.join("recent.dmp"), b"keep");

    let first = inspect(&p, &h);
    assert!(first.partial);
    let ids: Vec<_> = first
        .items
        .iter()
        .filter(|i| i.assessment == Assessment::Candidate)
        .map(|i| i.id.clone())
        .collect();
    assert_eq!(ids.len(), 100);
    let result = apply(&first, ids, &h).unwrap();
    assert!(
        result
            .items
            .iter()
            .all(|i| matches!(i.removal, Some(Removal::Removed)))
    );
    let second = inspect(&p, &h);
    assert_eq!(
        second
            .items
            .iter()
            .filter(|i| i.assessment == Assessment::Candidate)
            .count(),
        5
    );
    assert!(pending.join("recent.dmp").is_file());
}
#[test]
fn selected_leaf_does_not_widen_to_its_parent_cache_and_exact_reports_need_no_sibling_scan() {
    let (_d, h, p) = fixture();
    let cache = h.paths.home.join("Library/Caches/Codex/Default/Cache");
    write(&cache.join("file"), b"keep");

    let r = execute_with(
        Request::Inspect {
            project_root: p.to_str().unwrap().into(),
            paths: vec![cache.join("file").to_str().unwrap().into()],
            keep_paths: vec![],
            product_temporary: false,
            codex: true,
        },
        &|| false,
        &h,
    )
    .unwrap();
    assert!(r.items.iter().all(|i| i.path != cache.to_str().unwrap()));
    assert!(
        r.items
            .iter()
            .all(|i| i.assessment != Assessment::Candidate)
    );
    let pending = h
        .paths
        .home
        .join("Library/Application Support/Codex/Crashpad/pending");
    for i in 0..1002 {
        write(&pending.join(format!("recent-{i}")), b"recent");
    }
    let target = pending.join("selected-old.dmp");
    write(&target, b"old");
    old(&target);
    let r = execute_with(
        Request::Inspect {
            project_root: p.to_str().unwrap().into(),
            paths: vec![target.to_str().unwrap().into()],
            keep_paths: vec![],
            product_temporary: false,
            codex: true,
        },
        &|| false,
        &h,
    )
    .unwrap();
    assert!(!r.partial);
    assert_eq!(r.items.len(), 1);
    assert_eq!(r.items[0].assessment, Assessment::Candidate);
}
