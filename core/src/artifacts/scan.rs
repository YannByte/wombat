use super::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, time::UNIX_EPOCH};
pub(super) struct Budget<'a> {
    pub cancelled: &'a dyn Fn() -> bool,
    pub until: Instant,
    pub partial: bool,
    pub gaps: usize,
    entries: usize,
    path_bytes: usize,
    pub git: BTreeMap<PathBuf, projects::Git>,
    pub codex_process: BTreeMap<(bool, bool), UseState>,
    pub codex_build: Option<Option<u64>>,
    pub daemon_locked: bool,
    pub daemon_lock_identity: Option<String>,
}
impl<'a> Budget<'a> {
    pub fn new(cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            cancelled,
            until: Instant::now() + Duration::from_secs(8),
            partial: false,
            gaps: 0,
            entries: 0,
            path_bytes: 0,
            git: BTreeMap::new(),
            codex_process: BTreeMap::new(),
            codex_build: None,
            daemon_locked: false,
            daemon_lock_identity: None,
        }
    }
    pub fn mark_partial(&mut self) {
        self.partial = true;
        self.gaps = self.gaps.saturating_add(1);
    }
    pub fn check(&self) -> Result<()> {
        if (self.cancelled)() {
            Err(operation_error(
                "CANCELLED",
                "Artifact inspection cancelled",
            ))
        } else {
            Ok(())
        }
    }
    pub fn metadata(&mut self) -> Result<bool> {
        self.check()?;
        if self.entries >= 100_000 || Instant::now() >= self.until {
            self.mark_partial();
            return Ok(false);
        }
        self.entries += 1;
        Ok(true)
    }
    pub fn path(&mut self, path: &Path) -> Result<bool> {
        if !self.metadata()? {
            return Ok(false);
        }
        self.path_bytes = self
            .path_bytes
            .saturating_add(path.as_os_str().as_encoded_bytes().len());
        if self.path_bytes > 8 * 1024 * 1024 {
            self.mark_partial();
            return Ok(false);
        }
        Ok(true)
    }
    pub fn probe_deadline(&self) -> Instant {
        (Instant::now() + Duration::from_secs(2)).min(self.until)
    }
}
fn regular_bytes(path: &Path) -> Option<Vec<u8>> {
    let m = fs::symlink_metadata(path).ok()?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > 256 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 256 * 1024).then_some(bytes)
}
fn regeneration(target: &projects::Target) -> (Regeneration, Option<String>) {
    if let Some(version) = cargo_regeneration(target) {
        return (Regeneration::CargoCache, Some(version));
    }
    if let Some(version) = typescript_regeneration(target) {
        return (Regeneration::TypeScriptOutput, Some(version));
    }
    (Regeneration::Unknown, None)
}
fn cargo_regeneration(target: &projects::Target) -> Option<String> {
    let owner = target.owner.as_ref()?;
    if target.path != owner.join("target") {
        return None;
    }
    let manifest = regular_bytes(&owner.join("Cargo.toml"))?;
    let tag = regular_bytes(&target.path.join("CACHEDIR.TAG"))?;
    let document = std::str::from_utf8(&manifest)
        .ok()?
        .parse::<toml_edit::DocumentMut>()
        .ok()?;
    let configured = owner.ancestors().any(|p| {
        [".cargo/config", ".cargo/config.toml"]
            .iter()
            .any(|c| p.join(c).exists())
    }) || ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"]
        .iter()
        .any(|k| crate::host_environment::get(k).is_some());
    if !(document.contains_key("package") || document.contains_key("workspace"))
        || !tag.starts_with(b"Signature: 8a477f597d28d172789f06886806bc55")
        || configured
    {
        return None;
    }
    let lock = regular_bytes(&owner.join("Cargo.lock"));
    Some(crate::hash(
        serde_json::to_vec(&(manifest, tag, lock)).ok()?,
    ))
}
fn typescript_regeneration(target: &projects::Target) -> Option<String> {
    let owner = target.owner.as_ref()?;
    let manifest = regular_bytes(&owner.join("package.json"))?;
    let package: serde_json::Value = serde_json::from_slice(&manifest).ok()?;
    // Compiler declarations are data. Never execute a discovered script or override.
    let args = shlex::split(package["scripts"]["build"].as_str()?)?;
    if !args.first().is_some_and(|s| s == "tsc")
        || !(args.len() == 1 || args.len() == 3 && matches!(args[1].as_str(), "-p" | "--project"))
    {
        return None;
    }
    let config_name = args.get(2).map_or("tsconfig.json", String::as_str);
    if Path::new(config_name).is_absolute()
        || Path::new(config_name)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    let config_path = owner.join(config_name);
    if projects::linked(&config_path) {
        return None;
    }
    let raw = regular_bytes(&config_path)?;
    let config: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let out = config["compilerOptions"]["outDir"].as_str()?;
    let output = crate::absolute(config_path.parent()?.join(out)).ok()?;
    if config.get("extends").is_some()
        || config["compilerOptions"]["noEmit"] == true
        || output != target.path
    {
        return None;
    }
    Some(crate::hash(serde_json::to_vec(&(manifest, raw)).ok()?))
}
#[derive(Default)]
pub(super) struct Tree {
    files: BTreeMap<String, u64>,
    allocated: Option<u64>,
    fingerprint: String,
    complete: bool,
    reasons: Vec<Reason>,
}
fn add_reason(reasons: &mut Vec<Reason>, reason: Reason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}
fn tree(root: &Path, b: &mut Budget<'_>, assets: bool) -> Result<Tree> {
    let mut result = Tree {
        complete: true,
        allocated: Some(0),
        ..Default::default()
    };
    let mut stack = vec![root.to_owned()];
    let mut records = BTreeMap::new();
    while let Some(path) = stack.pop() {
        if !b.metadata()? {
            result.complete = false;
            add_reason(&mut result.reasons, Reason::ScanLimit);
            break;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                result.complete = false;
                add_reason(&mut result.reasons, Reason::ReadUnavailable);
                continue;
            }
        };
        let relative = path.strip_prefix(root).unwrap();
        let Some(name) = relative.to_str() else {
            result.complete = false;
            add_reason(&mut result.reasons, Reason::ReadUnavailable);
            continue;
        };
        if metadata.file_type().is_symlink() {
            if assets {
                add_reason(&mut result.reasons, Reason::Symlink);
            } else {
                #[cfg(not(windows))]
                let link_identity =
                    crate::file_metadata::FileMetadata::from_metadata(metadata.clone()).physical;
                #[cfg(windows)]
                let link_identity = format!("{:?}", metadata.created().ok());
                records.insert(
                    name.to_owned(),
                    (
                        format!(
                            "link:{link_identity}:{:?}:{:?}",
                            fs::read_link(&path).ok(),
                            metadata.modified().ok()
                        ),
                        metadata.len(),
                        metadata
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|t| t.as_nanos().to_string()),
                        format!("{:?}", metadata.created().ok()),
                        false,
                    ),
                );
            }
            continue;
        }
        if !metadata.is_dir() && !metadata.is_file() {
            add_reason(&mut result.reasons, Reason::SpecialFile);
            continue;
        }
        let physical = match crate::file_metadata::FileMetadata::read(&path) {
            Ok(m) => m.physical,
            Err(_) => {
                result.complete = false;
                add_reason(&mut result.reasons, Reason::ReadUnavailable);
                continue;
            }
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos().to_string());
        #[cfg(unix)]
        let changed = {
            use std::os::unix::fs::MetadataExt;
            format!("{}:{}", metadata.ctime(), metadata.ctime_nsec())
        };
        #[cfg(not(unix))]
        let changed = format!("{:?}", metadata.created().ok());
        records.insert(
            name.to_owned(),
            (
                physical.clone(),
                metadata.len(),
                modified,
                changed,
                metadata.is_dir(),
            ),
        );
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
        if assets && name == ".git" {
            add_reason(&mut result.reasons, Reason::NestedRepository);
            continue;
        }
        if assets
            && (matches!(
                name,
                "AGENTS.md"
                    | "SKILL.md"
                    | "MEMORY.md"
                    | "auth.json"
                    | "credentials"
                    | "credentials.json"
                    | ".env"
            ) || name.starts_with(".env."))
        {
            add_reason(&mut result.reasons, Reason::UserAsset);
        } else if assets
            && metadata.is_file()
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("md" | "txt" | "jsonl")
            )
            && name != "CACHEDIR.TAG"
        {
            add_reason(&mut result.reasons, Reason::UntrackedContent);
        }
        if metadata.is_dir() {
            match fs::read_dir(&path) {
                Ok(entries) => {
                    for e in entries {
                        match e {
                            Ok(e) => {
                                let p = e.path();
                                if !b.path(&p)? {
                                    result.complete = false;
                                    add_reason(&mut result.reasons, Reason::ScanLimit);
                                    break;
                                }
                                stack.push(p);
                            }
                            Err(_) => {
                                result.complete = false;
                                add_reason(&mut result.reasons, Reason::ReadUnavailable);
                            }
                        }
                    }
                }
                Err(_) => {
                    result.complete = false;
                    add_reason(&mut result.reasons, Reason::ReadUnavailable);
                }
            }
        } else if result.files.insert(physical, metadata.len()).is_none() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                result.allocated = result
                    .allocated
                    .and_then(|n| n.checked_add(metadata.blocks().checked_mul(512)?));
            }
            #[cfg(not(unix))]
            {
                result.allocated = None;
            }
        }
    }
    let mut hash = Sha256::new();
    for row in records {
        hash.update(serde_json::to_vec(&row)?);
    }
    result.fingerprint = format!("{:x}", hash.finalize());
    Ok(result)
}
pub(super) fn inspect(
    scope: &Scope,
    target: projects::Target,
    b: &mut Budget<'_>,
    host: &dyn projects::Host,
    files: &mut BTreeMap<String, u64>,
) -> Result<Item> {
    let gap_start = b.gaps;
    let path = target.path.clone();
    let owner = target
        .owner
        .as_ref()
        .map(|p| projects::text(p))
        .transpose()?;
    let mut item = Item {
        id: crate::hash(projects::text(&path)?),
        path: projects::text(&path)?,
        owner,
        kind: target.kind.clone(),
        assessment: Assessment::Review,
        regeneration: Regeneration::Unknown,
        regeneration_version: None,
        use_state: UseState::Unknown,
        observation: None,
        reasons: vec![],
        recheck: None,
        codex_evidence: None,
        removal: None,
    };
    let root = Path::new(&scope.project_root);
    let data = crate::storage::data_home()?;
    let kept = scope
        .keep_paths
        .iter()
        .any(|keep| path.starts_with(keep) || Path::new(keep).starts_with(&path));
    if kept
        || path == root
        || !super::codex::is_kind(&target.kind)
            && (projects::protected(&path) || super::codex::protected(&path, &host.codex_paths()))
        || target.kind != Kind::ProductTemporary
            && (path.starts_with(&data) || data.starts_with(&path))
    {
        item.assessment = Assessment::Protected;
        item.reasons.push(if kept {
            Reason::UserKept
        } else {
            Reason::UserAsset
        });
        return Ok(item);
    }
    if projects::linked(&path) {
        item.assessment = Assessment::Protected;
        item.reasons.push(Reason::Symlink);
        return Ok(item);
    }
    let physical = match crate::file_metadata::FileMetadata::read(&path) {
        Ok(m) => m.physical,
        Err(e) => {
            item.assessment = if e.kind() == std::io::ErrorKind::NotFound {
                Assessment::Absent
            } else {
                Assessment::Unknown
            };
            item.reasons
                .push(if e.kind() == std::io::ErrorKind::NotFound {
                    Reason::NotFound
                } else {
                    Reason::ReadUnavailable
                });
            return Ok(item);
        }
    };
    let mut first = tree(&path, b, !super::codex::is_kind(&target.kind))?;
    item.use_state = host.usage(&path, b);
    if target.kind == Kind::ProductTemporary {
        // A writer can temporarily close a stage file while still owning its generation.
        // Read the existing lock without creating product data or acquiring a write lock.
        let lock = data.join("usage-v4/refresh.lock");
        let lock_state = if projects::linked(&lock) {
            UseState::Unknown
        } else {
            match fs::File::open(lock)
                .and_then(|f| f.try_lock_shared().map(|_| f).map_err(std::io::Error::from))
            {
                Ok(_guard) => UseState::NoObservedOpenFiles,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => UseState::InUse,
                Err(_) => UseState::Unknown,
            }
        };
        if lock_state == UseState::InUse || item.use_state == UseState::InUse {
            item.use_state = UseState::InUse;
        } else if lock_state == UseState::Unknown {
            item.use_state = UseState::Unknown;
        }
    }
    let second = tree(&path, b, !super::codex::is_kind(&target.kind))?;
    if first.fingerprint != second.fingerprint || !second.complete {
        first.complete = false;
        add_reason(&mut first.reasons, Reason::ChangedDuringScan);
    }
    item.reasons = first.reasons;
    if super::codex::is_kind(&target.kind) {
        super::codex::assess(&target, &mut item, b, host)?;
    } else if target.kind == Kind::BuildOutput {
        (item.regeneration, item.regeneration_version) = regeneration(&target);
        if item.regeneration != Regeneration::Unknown {
            add_reason(&mut item.reasons, Reason::DeclaredBuildOutput);
        } else {
            add_reason(&mut item.reasons, Reason::RegenerationUnknown);
        }
        if !b.git.contains_key(root) {
            let git = host.git(root, b);
            b.git.insert(root.to_owned(), git);
        }
        let git = &b.git[root];
        if !git.available {
            add_reason(&mut item.reasons, Reason::GitUnavailable);
        } else if !git.repository {
            add_reason(&mut item.reasons, Reason::NoRepository);
        }
        if git.tracked.iter().any(|p| p.starts_with(&path)) {
            add_reason(&mut item.reasons, Reason::TrackedContent);
        }
        if git.untracked.iter().any(|p| p.starts_with(&path)) {
            add_reason(&mut item.reasons, Reason::UntrackedContent);
        }
    } else {
        add_reason(
            &mut item.reasons,
            if target.kind == Kind::ProductTemporary {
                Reason::ProductStage
            } else {
                Reason::TaskOwnershipRequired
            },
        );
    }
    if b.gaps != gap_start {
        first.complete = false;
        add_reason(&mut item.reasons, Reason::ScanLimit);
    }
    match item.use_state {
        UseState::InUse => add_reason(&mut item.reasons, Reason::InUse),
        UseState::Unknown => add_reason(&mut item.reasons, Reason::UseUnknown),
        UseState::NoObservedOpenFiles => {}
    }
    if item.reasons.iter().any(|r| {
        matches!(
            r,
            Reason::InUse
                | Reason::TrackedContent
                | Reason::NestedRepository
                | Reason::UserAsset
                | Reason::Symlink
                | Reason::SpecialFile
        )
    }) {
        item.assessment = Assessment::Protected;
    } else if !first.complete
        || item
            .reasons
            .iter()
            .any(|r| matches!(r, Reason::UseUnknown | Reason::GitUnavailable))
    {
        item.assessment = Assessment::Unknown;
    } else if !super::codex::is_kind(&target.kind)
        && item.regeneration != Regeneration::Unknown
        && !item
            .reasons
            .iter()
            .any(|r| matches!(r, Reason::UntrackedContent | Reason::NoRepository))
    {
        item.assessment = Assessment::Candidate;
    }
    let logical = first.files.values().try_fold(0_u64, |sum, n| {
        sum.checked_add(*n)
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Artifact size overflow"))
    })?;
    item.observation = Some(Observation {
        physical_identity: physical.clone(),
        fingerprint: first.fingerprint,
        logical_bytes: logical,
        allocated_bytes: first.allocated,
        files: first.files.len(),
        complete: first.complete,
    });
    item.id = crate::hash(serde_json::to_vec(&(&item.path, physical))?);
    for (id, size) in first.files {
        files.entry(id).or_insert(size);
    }
    Ok(item)
}
