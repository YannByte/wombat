//! Codex-owned disposable objects. Directory names alone never authorize removal.
use super::*;
use std::{ffi::OsStr, fs, io::Read, time::SystemTime};

pub(super) const AGE_DAYS: u32 = 30;
#[derive(Clone)]
pub(super) struct Paths {
    pub home: PathBuf,
    pub codex: PathBuf,
    pub applications: Vec<PathBuf>,
    pub supported: bool,
}
impl Paths {
    pub fn native() -> Self {
        let home = crate::home();
        let codex = crate::host_environment::get("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        let codex = crate::absolute(&codex).unwrap_or_default();
        let supported = cfg!(target_os = "macos") && codex.is_absolute();
        Self {
            applications: vec![
                PathBuf::from("/Applications/Codex.app"),
                home.join("Applications/Codex.app"),
            ],
            home,
            codex,
            supported,
        }
    }
    fn daemon(&self) -> PathBuf {
        self.codex.join("packages/app-server-daemon")
    }
    fn desktop(&self) -> PathBuf {
        self.home.join("Library/Caches/Codex")
    }
    fn updates(&self) -> PathBuf {
        self.home
            .join("Library/Caches/com.openai.codex/org.sparkle-project.Sparkle/Installation")
    }
    fn crash(&self) -> PathBuf {
        self.home
            .join("Library/Application Support/Codex/Crashpad/pending")
    }
    fn runtimes(&self) -> PathBuf {
        self.home.join(".cache/codex-runtimes")
    }
    fn markets(&self) -> [PathBuf; 2] {
        [
            self.codex.join(".tmp/bundled-marketplaces"),
            self.codex.join(".tmp/marketplaces/.staging"),
        ]
    }
}
pub(super) fn is_kind(k: &Kind) -> bool {
    matches!(
        k,
        Kind::CodexDesktopCache
            | Kind::CodexUpdateStage
            | Kind::CodexMarketplaceStage
            | Kind::CodexDaemonRelease
            | Kind::CodexRuntime
            | Kind::CodexCrashReport
    )
}
fn name(path: &Path) -> &str {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("")
}
fn release_name(s: &str) -> bool {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN
        .get_or_init(|| {
            regex::Regex::new(
                r"^(?:[0-9]+\.[0-9]+\.[0-9]+[-A-Za-z0-9._]*|local-[0-9a-f]{8,}-[A-Za-z0-9._-]+)$",
            )
            .unwrap()
        })
        .is_match(s)
}
fn selected(scope: &Scope, path: &Path) -> bool {
    scope.paths.is_empty() || scope.paths.iter().any(|s| path.starts_with(s))
}
fn stage_name(s: &str) -> bool {
    [
        "openai-bundled.staging-",
        "marketplace-upgrade-",
        "marketplace-add-",
    ]
    .iter()
    .any(|p| s.starts_with(p) && s.len() > p.len())
}
fn children(root: &Path, b: &mut scan::Budget<'_>) -> Result<Vec<PathBuf>> {
    if projects::linked(root) {
        b.mark_partial();
        return Ok(vec![root.to_owned()]);
    }
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => {
            b.mark_partial();
            return Ok(vec![root.to_owned()]);
        }
    };
    let mut out = vec![];
    for e in entries {
        if !b.metadata()? {
            break;
        }
        match e {
            Ok(e) => {
                let p = e.path();
                if !b.path(&p)? {
                    break;
                }
                out.push(p);
                if out.len() > 1000 {
                    b.mark_partial();
                    break;
                }
            }
            Err(_) => b.mark_partial(),
        }
    }
    out.sort();
    Ok(out)
}
pub(super) fn discover(
    scope: &Scope,
    b: &mut scan::Budget<'_>,
    p: &Paths,
) -> Result<Vec<projects::Target>> {
    if !p.supported {
        b.mark_partial();
        return Ok(vec![]);
    }
    let mut out = vec![];
    let mut add = |path: PathBuf, kind: Kind, owner: PathBuf, b: &mut scan::Budget<'_>| {
        if !selected(scope, &path) {
            return;
        }
        if out.iter().any(|t: &projects::Target| t.path == path) {
            return;
        }
        if out.len() >= MAX_ITEMS {
            b.mark_partial();
            return;
        }
        if fs::symlink_metadata(&path).is_ok() {
            out.push(projects::Target {
                path,
                kind,
                owner: Some(owner),
            });
        }
    };
    // Exact selected leaves need no inventory of their siblings.
    for path in &scope.paths {
        let path = PathBuf::from(path);
        for kind in [
            Kind::CodexDesktopCache,
            Kind::CodexUpdateStage,
            Kind::CodexMarketplaceStage,
            Kind::CodexDaemonRelease,
            Kind::CodexRuntime,
            Kind::CodexCrashReport,
        ] {
            let owner = match kind {
                Kind::CodexDesktopCache => p.desktop(),
                Kind::CodexUpdateStage => p.updates(),
                Kind::CodexDaemonRelease => p.daemon().join("releases"),
                Kind::CodexRuntime => p.runtimes(),
                Kind::CodexCrashReport => p.crash(),
                Kind::CodexMarketplaceStage => path.parent().unwrap_or(&path).to_owned(),
                _ => unreachable!(),
            };
            let target = projects::Target {
                path: path.clone(),
                kind: kind.clone(),
                owner: Some(owner.clone()),
            };
            if allowed(&target, p) {
                add(path.clone(), kind, owner, b);
                break;
            }
        }
    }
    for profile in [
        "Default",
        "Default/Partitions/codex-browser-app",
        "codex-browser-app",
    ] {
        for leaf in ["Cache", "Code Cache"] {
            add(
                p.desktop().join(profile).join(leaf),
                Kind::CodexDesktopCache,
                p.desktop(),
                b,
            );
        }
    }
    for path in if selected(scope, &p.updates()) {
        children(&p.updates(), b)?
    } else {
        vec![]
    } {
        add(path, Kind::CodexUpdateStage, p.updates(), b);
    }
    for root in p.markets() {
        for path in if selected(scope, &root) {
            children(&root, b)?
        } else {
            vec![]
        } {
            if stage_name(name(&path)) || path == root {
                add(path, Kind::CodexMarketplaceStage, root.clone(), b);
            }
        }
    }
    let releases = p.daemon().join("releases");
    for path in if selected(scope, &releases) {
        children(&releases, b)?
    } else {
        vec![]
    } {
        if release_name(name(&path)) || path == releases {
            add(path, Kind::CodexDaemonRelease, releases.clone(), b);
        }
    }
    for path in if selected(scope, &p.runtimes()) {
        children(&p.runtimes(), b)?
    } else {
        vec![]
    } {
        add(path, Kind::CodexRuntime, p.runtimes(), b);
    }
    if selected(scope, &p.crash()) {
        let root = p.crash();
        if projects::linked(&root) {
            add(root.clone(), Kind::CodexCrashReport, root, b);
        } else {
            match fs::read_dir(&root) {
                Ok(entries) => {
                    let mut n = 0;
                    for entry in entries {
                        if !b.metadata()? {
                            break;
                        }
                        let path = match entry {
                            Ok(e) => e.path(),
                            Err(_) => {
                                b.mark_partial();
                                continue;
                            }
                        };
                        match fs::symlink_metadata(&path) {
                            Ok(m) if m.is_file() && !m.file_type().is_symlink() && old(&path) => {
                                add(path, Kind::CodexCrashReport, root.clone(), b);
                                n += 1;
                                if n >= MAX_ITEMS {
                                    b.mark_partial();
                                    break;
                                }
                            }
                            Ok(_) => {}
                            Err(_) => b.mark_partial(),
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    b.mark_partial();
                    add(root.clone(), Kind::CodexCrashReport, root, b);
                }
            }
        }
    }
    for protected_root in [&p.codex, &p.home.join("Library/Application Support/Codex")] {
        add(
            protected_root.clone(),
            Kind::SelectedTemporary,
            protected_root.clone(),
            b,
        );
    }
    for selected in &scope.paths {
        let path = Path::new(selected);
        if !out
            .iter()
            .any(|t| t.path.starts_with(path) || path.starts_with(&t.path))
            && fs::symlink_metadata(path).is_ok()
            && out.len() < MAX_ITEMS
        {
            out.push(projects::Target {
                path: path.to_owned(),
                kind: Kind::SelectedTemporary,
                owner: None,
            });
        }
    }
    Ok(out)
}
fn bounded(path: &Path) -> Option<Vec<u8>> {
    if projects::linked(path) {
        return None;
    }
    let m = fs::symlink_metadata(path).ok()?;
    if !m.is_file() || m.len() > 65536 {
        return None;
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .ok()?
        .take(65537)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 65536).then_some(bytes)
}
fn old(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|d| d.as_secs() > u64::from(AGE_DAYS) * 86400)
}
fn keep(p: &Paths) -> Option<(String, Option<String>, String)> {
    let root = p.daemon();
    let releases = root.join("releases");
    if projects::linked(&releases)
        || !fs::symlink_metadata(root.join("current"))
            .ok()?
            .file_type()
            .is_symlink()
    {
        return None;
    }
    let current = dunce::canonicalize(root.join("current")).ok()?;
    if current.parent() != Some(releases.as_path()) || !release_name(name(&current)) {
        return None;
    }
    let marker = root.join("auto-update-version");
    let pending = match fs::symlink_metadata(&marker) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Ok(_) => {
            let bytes = bounded(&marker)?;
            let s = std::str::from_utf8(&bytes)
                .ok()?
                .trim_end_matches(['\r', '\n']);
            if !release_name(s) {
                return None;
            }
            Some(s.to_owned())
        }
        Err(_) => return None,
    };
    let stamp = crate::hash(
        serde_json::to_vec(&(
            crate::file_metadata::FileMetadata::read(&releases)
                .ok()?
                .physical,
            fs::read_link(root.join("current")).ok()?,
            &pending,
        ))
        .ok()?,
    );
    Some((name(&current).into(), pending, stamp))
}
pub(super) fn lock(p: &Paths) -> std::result::Result<fs::File, Reason> {
    let root = p.daemon();
    let path = root.join("install.lock");
    if projects::linked(&path) || fs::symlink_metadata(root.join("install.lock.d")).is_ok() {
        return Err(Reason::InstallationUnknown);
    }
    let m = fs::symlink_metadata(&path).map_err(|_| Reason::InstallationUnknown)?;
    if !m.is_file() {
        return Err(Reason::InstallationUnknown);
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options
        .open(&path)
        .map_err(|_| Reason::InstallationUnknown)?;
    let opened = crate::file_metadata::FileMetadata::from_file(&file)
        .map_err(|_| Reason::InstallationUnknown)?
        .physical;
    if projects::linked(&path)
        || crate::file_metadata::FileMetadata::read(&path)
            .map_err(|_| Reason::InstallationUnknown)?
            .physical
            != opened
    {
        return Err(Reason::InstallationUnknown);
    }
    file.try_lock().map_err(|e| {
        if std::io::Error::from(e).kind() == std::io::ErrorKind::WouldBlock {
            Reason::InstallationBusy
        } else {
            Reason::InstallationUnknown
        }
    })?;
    Ok(file)
}
fn allowed(t: &projects::Target, p: &Paths) -> bool {
    match t.kind {
        Kind::CodexDesktopCache => [
            "Default",
            "Default/Partitions/codex-browser-app",
            "codex-browser-app",
        ]
        .iter()
        .any(|profile| {
            ["Cache", "Code Cache"]
                .iter()
                .any(|leaf| t.path == p.desktop().join(profile).join(leaf))
        }),
        Kind::CodexUpdateStage => t.path.parent() == Some(p.updates().as_path()),
        Kind::CodexMarketplaceStage => p
            .markets()
            .iter()
            .any(|root| t.path.parent() == Some(root) && stage_name(name(&t.path))),
        Kind::CodexDaemonRelease => {
            t.path.parent() == Some(p.daemon().join("releases").as_path())
                && release_name(name(&t.path))
        }
        Kind::CodexRuntime => t.path.parent() == Some(p.runtimes().as_path()),
        Kind::CodexCrashReport => t.path.parent() == Some(p.crash().as_path()),
        _ => false,
    }
}
fn block(item: &mut Item, a: Assessment, r: Reason) {
    item.assessment = a;
    item.reasons.push(r);
}
pub(super) fn assess(
    t: &projects::Target,
    item: &mut Item,
    b: &mut scan::Budget<'_>,
    host: &dyn projects::Host,
) -> Result<()> {
    let p = host.codex_paths();
    if !p.supported {
        block(item, Assessment::Unknown, Reason::UnsupportedPlatform);
        return Ok(());
    }
    if !allowed(t, &p) {
        block(item, Assessment::Protected, Reason::UserAsset);
        return Ok(());
    }
    let m = match fs::symlink_metadata(&t.path) {
        Ok(m) => m,
        Err(_) => {
            block(item, Assessment::Unknown, Reason::ReadUnavailable);
            return Ok(());
        }
    };
    if (t.kind == Kind::CodexCrashReport && !m.is_file())
        || (t.kind != Kind::CodexCrashReport && !m.is_dir())
    {
        block(item, Assessment::Protected, Reason::SpecialFile);
        return Ok(());
    }
    let Some(parent_identity) = crate::file_metadata::FileMetadata::read(t.path.parent().unwrap())
        .ok()
        .map(|m| m.physical)
    else {
        block(item, Assessment::Unknown, Reason::ReadUnavailable);
        return Ok(());
    };
    let Some(owner_identity) = t
        .owner
        .as_ref()
        .and_then(|owner| crate::file_metadata::FileMetadata::read(owner).ok())
        .map(|m| m.physical)
    else {
        block(item, Assessment::Unknown, Reason::ReadUnavailable);
        return Ok(());
    };
    let mut evidence = CodexEvidence {
        install_lock_identity: None,
        parent_identity,
        minimum_age_days: None,
        current_release: None,
        pending_release: None,
        installed_build: None,
        staged_build: None,
    };
    let mut stamp = vec![owner_identity];
    item.assessment = Assessment::Candidate;
    match t.kind {
        Kind::CodexDaemonRelease => {
            let _guard = if b.daemon_locked {
                None
            } else {
                match lock(&p) {
                    Ok(f) => Some(f),
                    Err(r) => {
                        block(item, Assessment::Unknown, r);
                        return Ok(());
                    }
                }
            };
            let lock_path = p.daemon().join("install.lock");
            let lock_identity = crate::file_metadata::FileMetadata::read(&lock_path)?.physical;
            if b.daemon_locked && b.daemon_lock_identity.as_ref() != Some(&lock_identity) {
                block(item, Assessment::Unknown, Reason::InstallationUnknown);
                return Ok(());
            }
            evidence.install_lock_identity = Some(lock_identity);
            let Some((current, pending, version)) = keep(&p) else {
                block(item, Assessment::Unknown, Reason::InstallationUnknown);
                return Ok(());
            };
            if name(&t.path) == current || pending.as_deref() == Some(name(&t.path)) {
                block(
                    item,
                    Assessment::Protected,
                    if name(&t.path) == current {
                        Reason::ActiveVersion
                    } else {
                        Reason::PendingUpdate
                    },
                );
            } else {
                item.reasons.push(Reason::SupersededVersion);
                item.regeneration = Regeneration::SupersededPackage;
            }
            evidence.current_release = Some(current);
            evidence.pending_release = pending;
            stamp.push(version);
        }
        Kind::CodexRuntime => {
            let active = t.path.join("runtime.json").is_file()
                && (t.path.join("dependencies/node").is_dir()
                    || t.path.join("dependencies/python").is_dir());
            let n = name(&t.path);
            let incomplete = ["tmp", "temp", "incomplete", "partial"]
                .iter()
                .any(|s| n.starts_with(s))
                || [".tmp", ".incomplete", "-incomplete", ".partial"]
                    .iter()
                    .any(|s| n.ends_with(s))
                || (!t.path.join("runtime.json").exists() && !t.path.join("dependencies").exists());
            if active {
                block(item, Assessment::Protected, Reason::ActiveRuntime);
            } else if incomplete {
                item.regeneration = Regeneration::DiscardedStage;
                item.reasons.push(Reason::IncompleteRuntime);
            } else {
                block(item, Assessment::Review, Reason::RegenerationUnknown);
            }
            stamp.push(format!("{active}:{incomplete}"));
        }
        Kind::CodexUpdateStage => {
            evidence.installed_build = if let Some(v) = b.codex_build {
                v
            } else {
                let v = host.installed_build(&p, b);
                b.codex_build = Some(v);
                v
            };
            evidence.staged_build = staged_build(&t.path, b, host)?;
            match (evidence.installed_build, evidence.staged_build) {
                (Some(installed), Some(staged)) if staged > installed => {
                    block(item, Assessment::Protected, Reason::PendingUpdate)
                }
                (Some(_), Some(_)) => {
                    item.reasons.push(Reason::SupersededVersion);
                    item.regeneration = Regeneration::DiscardedStage;
                }
                _ => {
                    evidence.minimum_age_days = Some(AGE_DAYS);
                    if !old(&t.path) {
                        block(item, Assessment::Protected, Reason::Recent);
                    } else {
                        item.regeneration = Regeneration::DiscardedStage;
                    }
                }
            }
            stamp.push(format!(
                "{:?}:{:?}",
                evidence.installed_build, evidence.staged_build
            ));
        }
        Kind::CodexMarketplaceStage | Kind::CodexCrashReport => {
            evidence.minimum_age_days = Some(AGE_DAYS);
            if !old(&t.path) {
                block(item, Assessment::Protected, Reason::Recent);
            } else {
                item.regeneration = if t.kind == Kind::CodexCrashReport {
                    Regeneration::Diagnostic
                } else {
                    Regeneration::DiscardedStage
                };
            }
        }
        Kind::CodexDesktopCache => {
            item.regeneration = Regeneration::CodexCache;
            item.reasons.push(Reason::KnownCodexCache);
        }
        _ => unreachable!(),
    }
    if t.kind != Kind::CodexDaemonRelease {
        let runtime = matches!(t.kind, Kind::CodexRuntime | Kind::CodexMarketplaceStage);
        let crash = t.kind == Kind::CodexCrashReport;
        let key = (runtime, crash);
        let state = if let Some(v) = b.codex_process.get(&key) {
            v.clone()
        } else {
            let v = host.process(runtime, crash, b);
            b.codex_process.insert(key, v.clone());
            v
        };
        match state {
            UseState::InUse => block(item, Assessment::Protected, Reason::InUse),
            UseState::Unknown => block(item, Assessment::Unknown, Reason::ProcessUnknown),
            UseState::NoObservedOpenFiles => {}
        }
    }
    item.regeneration_version = Some(crate::hash(serde_json::to_vec(&(stamp, &evidence))?));
    item.codex_evidence = Some(evidence);
    Ok(())
}
fn staged_build(
    path: &Path,
    b: &mut scan::Budget<'_>,
    host: &dyn projects::Host,
) -> Result<Option<u64>> {
    let mut version = None;
    for app in children(path, b)? {
        if app.extension().is_some_and(|s| s == "app") {
            let Some(v) = host.bundle_build(&app, b) else {
                return Ok(None);
            };
            if version.is_some_and(|n| n != v) {
                return Ok(None);
            }
            version = Some(v);
        }
    }
    Ok(version)
}
pub(super) fn native_process(runtime: bool, crash: bool, b: &scan::Budget<'_>) -> UseState {
    #[cfg(unix)]
    {
        let mut probes = vec![("-x", "Codex|ChatGPT"), ("-f", "/(Codex|ChatGPT)[.]app/")];
        if runtime {
            probes.push(("-x", "codex"));
        }
        if crash {
            probes.push(("-f", "Application Support/Codex/Crashpad"));
        }
        probes.push((
            "-f",
            "org[.]sparkle-project[.]Sparkle|Sparkle[.]framework/.*/(Autoupdate|Installer|Downloader|Updater)",
        ));
        for (flag, pattern) in probes {
            match crate::host_process::probe(
                "/usr/bin/pgrep",
                &[OsStr::new(flag), OsStr::new(pattern)],
                b.cancelled,
                b.probe_deadline(),
            ) {
                Ok(o) if o.code == Some(0) => return UseState::InUse,
                Ok(o) if o.code == Some(1) && o.stderr.is_empty() => {}
                _ => return UseState::Unknown,
            }
        }
        UseState::NoObservedOpenFiles
    }
    #[cfg(not(unix))]
    {
        let _ = (runtime, crash, b);
        UseState::Unknown
    }
}
pub(super) fn bundle_build(app: &Path, b: &scan::Budget<'_>) -> Option<u64> {
    let plist = app.join("Contents/Info.plist");
    bounded(&plist)?;
    let o = crate::host_process::probe(
        "/usr/bin/plutil",
        &[
            OsStr::new("-convert"),
            OsStr::new("json"),
            OsStr::new("-o"),
            OsStr::new("-"),
            plist.as_os_str(),
        ],
        b.cancelled,
        b.probe_deadline(),
    )
    .ok()?;
    if o.code != Some(0) || !o.stderr.is_empty() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).ok()?;
    if v["CFBundleIdentifier"] != "com.openai.codex" {
        return None;
    }
    let s = v["CFBundleVersion"].as_str()?;
    if s.is_empty() || s.len() > 10 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}
pub(super) fn installed_build(p: &Paths, b: &scan::Budget<'_>) -> Option<u64> {
    let o = crate::host_process::probe(
        "/usr/bin/mdfind",
        &[OsStr::new(
            "kMDItemCFBundleIdentifier == 'com.openai.codex'",
        )],
        b.cancelled,
        b.probe_deadline(),
    )
    .ok()?;
    if o.code != Some(0) || !o.stderr.is_empty() {
        return None;
    }
    let text = std::str::from_utf8(&o.stdout).ok()?;
    let mut apps = p.applications.clone();
    for line in text.lines() {
        let app = PathBuf::from(line);
        if !app.is_absolute() || !app.extension().is_some_and(|e| e == "app") {
            continue;
        }
        if !app.starts_with(p.home.join("Library/Caches")) {
            apps.push(app);
        }
    }
    let mut build = None;
    for app in apps {
        if let Some(v) = bundle_build(&app, b) {
            if build.is_some_and(|n| n != v) {
                return None;
            }
            build = Some(v);
        }
    }
    build
}

pub(super) fn remove(item: &Item, cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        return Err(operation_error("CANCELLED", "Cleanup cancelled"));
    }
    let path = Path::new(&item.path);
    if !is_kind(&item.kind) || item.assessment != Assessment::Candidate || projects::linked(path) {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Cleanup target no longer qualifies",
        ));
    }
    let observed = item
        .observation
        .as_ref()
        .ok_or_else(|| operation_error("INVALID_ARGUMENT", "Missing cleanup observation"))?;
    if crate::file_metadata::FileMetadata::read(path)?.physical != observed.physical_identity
        || item.codex_evidence.as_ref().is_none_or(|e| {
            crate::file_metadata::FileMetadata::read(path.parent().unwrap())
                .ok()
                .is_none_or(|m| m.physical != e.parent_identity)
        })
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Cleanup target identity changed",
        ));
    }
    if item.kind == Kind::CodexDaemonRelease {
        let lock = Path::new(item.owner.as_ref().unwrap())
            .parent()
            .unwrap()
            .join("install.lock");
        if projects::linked(&lock)
            || item
                .codex_evidence
                .as_ref()
                .and_then(|e| e.install_lock_identity.as_ref())
                .is_none_or(|id| {
                    crate::file_metadata::FileMetadata::read(&lock)
                        .ok()
                        .is_none_or(|m| &m.physical != id)
                })
        {
            return Err(operation_error(
                "UPDATE_BUSY",
                "Codex installation lock changed",
            ));
        }
    }
    if item.kind == Kind::CodexCrashReport {
        fs::remove_file(path)?;
    } else {
        fs::remove_dir_all(path)?;
    }
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => Err(operation_error(
            "INVALID_ARGUMENT",
            "Cleanup path was recreated; inspect again",
        )),
    }
}

pub(super) fn protected(path: &Path, p: &Paths) -> bool {
    [
        p.codex.clone(),
        p.home.join("Library/Application Support/Codex"),
        p.home.join("Library/Logs/com.openai.codex"),
        p.home.join("Library/Caches/Codex"),
        p.home.join("Library/Caches/com.openai.codex"),
        p.runtimes(),
        p.home.join(".gemini/tmp"),
        p.home.join(".lmstudio"),
        p.home.join(".cache/lm-studio"),
    ]
    .iter()
    .any(|root| {
        let physical = dunce::canonicalize(root).unwrap_or_else(|_| root.clone());
        path.starts_with(root)
            || root.starts_with(path)
            || path.starts_with(&physical)
            || physical.starts_with(path)
    })
}
