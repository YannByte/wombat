use super::*;
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
};
struct Host {
    use_state: UseState,
    tracked: Vec<PathBuf>,
    untracked: Vec<PathBuf>,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            use_state: UseState::NoObservedOpenFiles,
            tracked: vec![],
            untracked: vec![],
        }
    }
}
impl projects::Host for Host {
    fn git(&self, _: &Path, _: &scan::Budget<'_>) -> projects::Git {
        projects::Git {
            tracked: self.tracked.iter().cloned().collect(),
            untracked: self.untracked.iter().cloned().collect(),
            available: true,
            repository: true,
        }
    }
    fn usage(&self, _: &Path, _: &scan::Budget<'_>) -> UseState {
        self.use_state.clone()
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='synthetic'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::create_dir(root.join("target")).unwrap();
    fs::write(
        root.join("target/CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55\n",
    )
    .unwrap();
    fs::write(root.join("target/object.o"), b"synthetic").unwrap();
    (dir, root)
}
fn request(root: &Path) -> Request {
    Request::Inspect {
        project_root: root.to_str().unwrap().into(),
        paths: vec![],
        keep_paths: vec![],
        product_temporary: false,
        codex: false,
    }
}
fn inspect(root: &Path, host: &Host) -> Response {
    execute_with(request(root), &|| false, host).unwrap()
}
fn recheck(before: &Response, host: &Host) -> Response {
    execute_with(
        Request::Recheck {
            baseline: Box::new(before.clone()),
            selection: vec![],
        },
        &|| false,
        host,
    )
    .unwrap()
}
#[test]
fn scan_is_read_only_and_known_cache_is_not_permission_to_remove() {
    let (_temp, root) = fixture();

    let before = fs::read(root.join("target/object.o")).unwrap();
    let response = inspect(&root, &Host::default());
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].assessment, Assessment::Candidate);
    assert_eq!(response.items[0].regeneration, Regeneration::CargoCache);
    assert!(!response.partial);
    assert_eq!(fs::read(root.join("target/object.o")).unwrap(), before);
    assert_eq!(
        recheck(&response, &Host::default()).items[0].recheck,
        Some(RecheckState::Unchanged)
    );
}
#[test]
fn tracked_ignored_personal_files_nested_repositories_and_pending_consumers_are_retained() {
    for (name, reason) in [
        ("note.md", Reason::UntrackedContent),
        (".env", Reason::UserAsset),
        (".git", Reason::NestedRepository),
    ] {
        let (_temp, root) = fixture();
        fs::write(root.join("target").join(name), b"synthetic").unwrap();
        let result = inspect(&root, &Host::default());
        assert_ne!(result.items[0].assessment, Assessment::Candidate);
        assert!(result.items[0].reasons.contains(&reason));
    }
    let (_temp, root) = fixture();
    let h = Host {
        tracked: vec![root.join("target/object.o")],
        ..Default::default()
    };
    assert_eq!(
        inspect(&root, &h).items[0].assessment,
        Assessment::Protected
    );
    let req = Request::Inspect {
        project_root: root.to_str().unwrap().into(),
        paths: vec![],
        keep_paths: vec![root.join("target").to_str().unwrap().into()],
        product_temporary: false,
        codex: false,
    };
    let r = execute_with(req, &|| false, &Host::default()).unwrap();
    assert_eq!(r.items[0].reasons, vec![Reason::UserKept]);
}
#[test]
fn missing_use_observations_never_establish_idle_or_disposable() {
    let (_temp, root) = fixture();
    for (state, assessment) in [
        (UseState::InUse, Assessment::Protected),
        (UseState::Unknown, Assessment::Unknown),
    ] {
        let h = Host {
            use_state: state,
            ..Default::default()
        };
        assert_eq!(inspect(&root, &h).items[0].assessment, assessment);
    }
}
#[test]
fn baseline_binds_original_selection_and_detects_changes_deletion_and_replacement() {
    let (_temp, root) = fixture();

    let h = Host::default();
    let before = inspect(&root, &h);
    fs::write(root.join("target/object.o"), b"changed").unwrap();
    assert_eq!(
        recheck(&before, &h).items[0].recheck,
        Some(RecheckState::Changed)
    );
    fs::remove_dir_all(root.join("target")).unwrap();
    assert_eq!(
        recheck(&before, &h).items[0].recheck,
        Some(RecheckState::Gone)
    );
    fs::create_dir(root.join("target")).unwrap();
    fs::write(root.join("target/new.o"), b"new").unwrap();
    assert_eq!(
        recheck(&before, &h).items[0].recheck,
        Some(RecheckState::Changed)
    );
    assert!(
        execute_with(
            Request::Recheck {
                baseline: Box::new(before.clone()),
                selection: vec!["unselected".into()]
            },
            &|| false,
            &h
        )
        .is_err()
    );
    assert!(
        execute_with(
            Request::Recheck {
                baseline: Box::new(Response {
                    baseline_id: "changed".into(),
                    ..before.clone()
                }),
                selection: vec![]
            },
            &|| false,
            &h
        )
        .is_err()
    );
}
#[test]
fn changed_regeneration_inputs_and_late_usage_invalidate_the_preview() {
    let (_temp, root) = fixture();

    let before = inspect(&root, &Host::default());
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='different'\nversion='0.2.0'\n",
    )
    .unwrap();
    assert_eq!(
        recheck(&before, &Host::default()).items[0].recheck,
        Some(RecheckState::Changed)
    );
    let busy = Host {
        use_state: UseState::InUse,
        ..Default::default()
    };
    assert_eq!(
        recheck(&before, &busy).items[0].assessment,
        Assessment::Protected
    );
}
#[test]
fn stateless_cancelled_and_incomplete_observations_do_not_become_success() {
    let (_temp, root) = fixture();

    let before = inspect(&root, &Host::default());
    // The full original plan survives independently of any process or timer.
    assert_eq!(
        recheck(&before, &Host::default()).items[0].recheck,
        Some(RecheckState::Unchanged)
    );
    assert!(execute_with(request(&root), &|| true, &Host::default()).is_err());
    let flag = AtomicBool::new(false);
    let cancelled = || flag.load(Ordering::Relaxed);
    let mut b = scan::Budget::new(&cancelled);
    b.until = Instant::now() - Duration::from_secs(1);
    let item = scan::inspect(
        &Scope {
            project_root: root.to_str().unwrap().into(),
            paths: vec![],
            keep_paths: vec![],
            product_temporary: false,
            codex: false,
        },
        projects::Target {
            path: root.join("target"),
            kind: Kind::BuildOutput,
            owner: Some(root.clone()),
        },
        &mut b,
        &Host::default(),
        &mut BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(item.assessment, Assessment::Unknown);
    assert!(!item.observation.unwrap().complete);
}
#[cfg(unix)]
#[test]
fn links_cannot_escape_scope_and_hard_links_are_counted_once() {
    use std::os::unix::fs::symlink;
    let (_temp, root) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("private"), b"must survive").unwrap();
    symlink(outside.path(), root.join("dist")).unwrap();
    let r = inspect(&root, &Host::default());
    assert!(
        r.items
            .iter()
            .any(|i| i.path.ends_with("dist") && i.assessment == Assessment::Protected)
    );
    assert_eq!(
        fs::read(outside.path().join("private")).unwrap(),
        b"must survive"
    );
    fs::hard_link(root.join("target/object.o"), root.join("target/alias.o")).unwrap();
    let r = inspect(&root, &Host::default());
    let item = r.items.iter().find(|i| i.path.ends_with("target")).unwrap();
    assert_eq!(item.observation.as_ref().unwrap().files, 2);
}
#[test]
fn selected_temporary_paths_need_review_without_recheck_expansion() {
    let (_temp, root) = fixture();
    fs::create_dir(root.join("temporary")).unwrap();
    fs::write(root.join("temporary/item.bin"), b"local").unwrap();

    let r = execute_with(
        Request::Inspect {
            project_root: root.to_str().unwrap().into(),
            paths: vec![root.join("temporary").to_str().unwrap().into()],
            keep_paths: vec![],
            product_temporary: false,
            codex: false,
        },
        &|| false,
        &Host::default(),
    )
    .unwrap();
    assert_eq!(r.items[0].assessment, Assessment::Review);
    assert!(r.items[0].reasons.contains(&Reason::TaskOwnershipRequired));
    fs::create_dir(root.join("new-output")).unwrap();
    assert_eq!(recheck(&r, &Host::default()).items.len(), 1);
}

#[test]
fn compiler_overrides_and_changed_owner_do_not_pass_original_conditions() {
    let (_temp, root) = fixture();
    fs::create_dir(root.join("dist")).unwrap();
    fs::write(root.join("dist/app.js"), b"compiled").unwrap();
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"outDir":"dist"}}"#,
    )
    .unwrap();
    fs::write(root.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let before = inspect(&root, &Host::default());
    let dist = before
        .items
        .iter()
        .find(|i| i.path.ends_with("dist"))
        .unwrap();
    assert_eq!(dist.assessment, Assessment::Candidate);
    fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"tsc --outDir other"}}"#,
    )
    .unwrap();
    let after = recheck(&before, &Host::default());
    let dist = after
        .items
        .iter()
        .find(|i| i.path.ends_with("dist"))
        .unwrap();
    assert_eq!(dist.regeneration, Regeneration::Unknown);
    assert_eq!(dist.recheck, Some(RecheckState::Changed));
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    assert!(
        recheck(&before, &Host::default())
            .items
            .iter()
            .all(|i| i.recheck == Some(RecheckState::Unavailable))
    );
    fs::remove_dir_all(&root).unwrap();
    fs::rename(moved, &root).unwrap();
}
#[cfg(unix)]
#[test]
fn explicitly_selected_external_links_are_not_traversed() {
    use std::os::unix::fs::symlink;
    let (_temp, root) = fixture();
    let external = tempfile::tempdir().unwrap();
    let external = dunce::canonicalize(external.path()).unwrap();
    fs::create_dir(external.join("actual")).unwrap();
    fs::write(external.join("actual/secret"), b"keep").unwrap();
    symlink(external.join("actual"), external.join("alias")).unwrap();
    let r = execute_with(
        Request::Inspect {
            project_root: root.to_str().unwrap().into(),
            paths: vec![external.join("alias/secret").to_str().unwrap().into()],
            keep_paths: vec![],
            product_temporary: false,
            codex: false,
        },
        &|| false,
        &Host::default(),
    )
    .unwrap();
    assert_eq!(r.items[0].assessment, Assessment::Protected);
    assert!(r.items[0].observation.is_none());
}

#[test]
fn selected_ids_bind_the_original_scope_and_evidence_even_if_plan_digest_is_recomputed() {
    let (_dir, root) = fixture();
    let original = inspect(&root, &Host::default());
    for mutate in [0, 1, 2] {
        let mut forged = original.clone();
        match mutate {
            0 => forged.scope.keep_paths.push("/different-scope".into()),
            1 => forged.items[0].path.push_str("-foreign"),
            _ => forged.items[0].assessment = Assessment::Protected,
        }
        forged.baseline_id = baseline_id(&forged).unwrap();
        let result = execute_with(
            Request::Recheck {
                baseline: Box::new(forged),
                selection: vec![original.items[0].id.clone()],
            },
            &|| false,
            &Host::default(),
        );
        assert!(result.is_err());
    }
    assert!(root.join("target/object.o").exists());
}
