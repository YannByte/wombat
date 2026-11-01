use super::*;
use std::{collections::BTreeSet, ffi::OsStr, fs};
pub(super) const OUTPUTS: &[&str] = &[
    "target",
    "dist",
    ".next",
    ".build",
    "build",
    "coverage",
    "__pycache__",
];
#[derive(Clone)]
pub(super) struct Target {
    pub path: PathBuf,
    pub kind: Kind,
    pub owner: Option<PathBuf>,
}
#[derive(Clone)]
pub(super) struct Git {
    pub tracked: BTreeSet<PathBuf>,
    pub untracked: BTreeSet<PathBuf>,
    pub available: bool,
    pub repository: bool,
}
pub(super) trait Host {
    fn git(&self, root: &Path, b: &scan::Budget<'_>) -> Git;
    fn usage(&self, path: &Path, b: &scan::Budget<'_>) -> UseState;
    fn codex_paths(&self) -> super::codex::Paths {
        super::codex::Paths::native()
    }
    fn process(&self, runtime: bool, crash: bool, b: &scan::Budget<'_>) -> UseState {
        super::codex::native_process(runtime, crash, b)
    }
    fn installed_build(&self, paths: &super::codex::Paths, b: &scan::Budget<'_>) -> Option<u64> {
        super::codex::installed_build(paths, b)
    }
    fn bundle_build(&self, app: &Path, b: &scan::Budget<'_>) -> Option<u64> {
        super::codex::bundle_build(app, b)
    }
}
pub(super) struct Native;
impl Host for Native {
    fn git(&self, root: &Path, b: &scan::Budget<'_>) -> Git {
        let mut result = Git {
            tracked: BTreeSet::new(),
            untracked: BTreeSet::new(),
            available: false,
            repository: false,
        };
        let run = |tail: &[&OsStr]| {
            let mut args = vec![
                OsStr::new("-c"),
                OsStr::new("core.fsmonitor=false"),
                OsStr::new("-c"),
                OsStr::new("core.untrackedCache=false"),
                OsStr::new("-C"),
                root.as_os_str(),
            ];
            args.extend_from_slice(tail);
            crate::host_process::probe("git", &args, b.cancelled, b.probe_deadline())
        };
        let Ok(repo) = run(&[OsStr::new("rev-parse"), OsStr::new("--show-toplevel")]) else {
            return result;
        };
        if repo.code != Some(0) {
            result.available = repo
                .stderr
                .windows(b"not a git repository".len())
                .any(|w| w == b"not a git repository");
            return result;
        }
        let Ok(name) = std::str::from_utf8(&repo.stdout) else {
            return result;
        };
        let base = PathBuf::from(name.trim_end_matches(['\r', '\n']));
        if !base.is_absolute() {
            return result;
        }
        result.repository = true;
        for flag in ["--cached", "--others"] {
            let Ok(output) = run(&[
                OsStr::new("ls-files"),
                OsStr::new("--full-name"),
                OsStr::new("-z"),
                OsStr::new(flag),
                OsStr::new("--exclude-standard"),
            ]) else {
                return result;
            };
            if output.code != Some(0) || !output.stderr.is_empty() {
                return result;
            }
            for path in output.stdout.split(|v| *v == 0).filter(|s| !s.is_empty()) {
                let Ok(path) = std::str::from_utf8(path) else {
                    return result;
                };
                let path = Path::new(path);
                if path.is_absolute()
                    || path
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    return result;
                }
                if flag == "--cached" {
                    result.tracked.insert(base.join(path));
                } else {
                    result.untracked.insert(base.join(path));
                }
            }
        }
        result.available = true;
        result
    }
    fn usage(&self, path: &Path, b: &scan::Budget<'_>) -> UseState {
        #[cfg(target_os = "macos")]
        let program = "/usr/sbin/lsof";
        #[cfg(all(unix, not(target_os = "macos")))]
        let program = "lsof";
        #[cfg(unix)]
        {
            let selector = if path.is_dir() { "+D" } else { "--" };
            let output = crate::host_process::probe(
                program,
                &[
                    OsStr::new("-nP"),
                    OsStr::new("-Fn"),
                    OsStr::new(selector),
                    path.as_os_str(),
                ],
                b.cancelled,
                b.probe_deadline(),
            );
            match output {
                Ok(o) if o.stderr.is_empty() && o.code == Some(1) && o.stdout.is_empty() => {
                    UseState::NoObservedOpenFiles
                }
                Ok(o) if o.code == Some(0) && !o.stdout.is_empty() => UseState::InUse,
                _ => UseState::Unknown,
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (path, b);
            UseState::Unknown
        }
    }
}
pub(super) fn text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| operation_error("INVALID_ARGUMENT", "Artifact paths must be valid Unicode"))
}
pub(super) fn scope_root(input: &str, codex: bool) -> Result<PathBuf> {
    let path = crate::absolute(input)?;
    if fs::symlink_metadata(&path)?.file_type().is_symlink() {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Select a physical project directory",
        ));
    }
    let path = dunce::canonicalize(path)?;
    if !path.is_dir() || path.parent().is_none() || !codex && path == crate::home() {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Select a project directory, not a filesystem or home root",
        ));
    }
    Ok(path)
}
fn has_manifest(root: &Path) -> bool {
    [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "Package.swift",
    ]
    .iter()
    .any(|p| root.join(p).is_file())
}
pub(super) fn owner(path: &Path, root: &Path) -> Option<PathBuf> {
    let mut at = path.parent()?;
    while at.starts_with(root) {
        if has_manifest(at) {
            return Some(at.to_owned());
        }
        at = at.parent()?;
    }
    None
}
pub(super) fn discover(
    scope: &Scope,
    b: &mut scan::Budget<'_>,
    host: &dyn Host,
) -> Result<Vec<Target>> {
    if scope.codex {
        return super::codex::discover(scope, b, &host.codex_paths());
    }
    let root = PathBuf::from(&scope.project_root);
    let mut targets = BTreeMap::new();
    if scope.paths.is_empty() {
        let mut todo = vec![(root.clone(), 0)];
        while let Some((dir, depth)) = todo.pop() {
            if !b.metadata()? {
                break;
            }
            if has_manifest(&dir) {
                for name in OUTPUTS {
                    let path = dir.join(name);
                    if fs::symlink_metadata(&path).is_ok() {
                        targets.insert(
                            path.clone(),
                            Target {
                                path,
                                kind: Kind::BuildOutput,
                                owner: Some(dir.clone()),
                            },
                        );
                    }
                }
            }
            if depth >= 3 {
                continue;
            }
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => {
                    b.mark_partial();
                    continue;
                }
            };
            for entry in entries {
                if !b.metadata()? {
                    break;
                }
                let Ok(entry) = entry else {
                    b.mark_partial();
                    continue;
                };
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    b.mark_partial();
                    continue;
                };
                if name.starts_with('.')
                    || OUTPUTS.contains(&name)
                    || matches!(name, "node_modules" | "vendor" | "venv" | "Pods")
                {
                    continue;
                }
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    let p = entry.path();
                    if !b.path(&p)? {
                        break;
                    }
                    todo.push((p, depth + 1));
                }
            }
        }
    } else {
        for path in &scope.paths {
            let path = PathBuf::from(path);
            let owner = owner(&path, &root);
            let build = path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| OUTPUTS.contains(&s))
                && owner.as_ref().is_some_and(|p| path.parent() == Some(p));
            targets.insert(
                path.clone(),
                Target {
                    path,
                    kind: if build {
                        Kind::BuildOutput
                    } else {
                        Kind::SelectedTemporary
                    },
                    owner,
                },
            );
        }
    }
    if scope.product_temporary {
        let data = crate::storage::data_home()?;
        let generations = data.join("usage-v4/generations");
        if linked(&generations) {
            b.mark_partial();
            return Ok(targets.into_values().take(MAX_ITEMS).collect());
        }
        let entries = match fs::read_dir(&generations) {
            Ok(entries) => Some(entries),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    b.mark_partial();
                }
                None
            }
        };
        if let Some(entries) = entries {
            for entry in entries {
                if !b.metadata()? {
                    break;
                }
                let Ok(entry) = entry else {
                    b.mark_partial();
                    continue;
                };
                if uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err() {
                    continue;
                }
                if entry.file_type().is_ok_and(|t| t.is_dir()) && !linked(&entry.path()) {
                    if let Ok(stages) = fs::read_dir(entry.path()) {
                        for stage in stages {
                            if !b.metadata()? {
                                break;
                            }
                            let Ok(stage) = stage else {
                                b.mark_partial();
                                continue;
                            };
                            if stage
                                .file_name()
                                .to_str()
                                .is_some_and(|s| s.starts_with(".pending-"))
                            {
                                let path = stage.path();
                                targets.insert(
                                    path.clone(),
                                    Target {
                                        path,
                                        kind: Kind::ProductTemporary,
                                        owner: Some(data.clone()),
                                    },
                                );
                            }
                        }
                    } else {
                        b.mark_partial();
                    }
                }
            }
        }
    }
    let mut out: Vec<Target> = vec![];
    for (_, target) in targets {
        if out.iter().any(|p| target.path.starts_with(&p.path)) {
            continue;
        }
        if out.len() == MAX_ITEMS {
            b.mark_partial();
            break;
        }
        out.push(target);
    }
    Ok(out)
}
pub(super) fn protected(path: &Path) -> bool {
    let home = crate::home();
    for source in [
        home.join(".claude/projects"),
        home.join(".claude/skills"),
        home.join(".claude/plans"),
    ] {
        let physical = dunce::canonicalize(&source).unwrap_or_else(|_| source.clone());
        if path.starts_with(&source)
            || source.starts_with(path)
            || path.starts_with(&physical)
            || physical.starts_with(path)
        {
            return true;
        }
    }
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| matches!(s, ".git" | ".agents" | ".aws" | ".ssh" | ".gnupg"))
    })
}
/// Never traverse an explicit path through a symlink, including paths outside the project.
pub(super) fn linked(path: &Path) -> bool {
    path.ancestors()
        .any(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
}
