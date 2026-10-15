//! Resident indexing shares the existing writer and committed read views.
use super::*;
use crate::service_dto::{Action, Request, Response};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(super) const PARSER_CACHE_BUDGET: u64 = 32 * 1024 * 1024;
pub(super) fn wal_pressure(db: &rusqlite::Connection) -> bool {
    db.path()
        .and_then(|p| fs::metadata(format!("{p}-wal")).ok())
        .is_some_and(|m| m.len() > 32 * 1024 * 1024)
}
/// Old live views retain exact cached answers under WAL pressure. Requests that
/// need released facts expire explicitly; saved snapshots remain independent.
pub(super) fn reclaim_wal(db: &rusqlite::Connection, shared: &Shared) -> Result<()> {
    let Ok(mut entries) = shared.0.try_lock() else {
        return Ok(());
    };
    if entries.is_empty() {
        return Ok(());
    }
    let pressure = wal_pressure(db);
    if pressure {
        for (key, entry) in entries.iter_mut() {
            let newest = entry.views.len().saturating_sub(1);
            let current = crate::live_index::load_map(db, &format!("view:{key}"))?;
            for (n, (_, view)) in entry.views.iter_mut().enumerate() {
                let mut matches = true;
                for source in &view.manifest.sources {
                    let epoch =
                        crate::live_index::load_map(db, &format!("epoch:{}", source.source.id))?;
                    matches &= current.get("epochs").and_then(|v| v.get(&source.source.id))
                        == Some(&Value::Object(epoch));
                }
                if (n < newest || !matches)
                    && let Some(view) = Arc::get_mut(view)
                {
                    view.freeze_cached_answers();
                }
            }
        }
    }
    for (key, entry) in entries.iter() {
        if (!pressure && entry.touched.elapsed() < Duration::from_secs(60))
            || entry.syncing
            || entry.restoring
            || entry.views.is_empty()
        {
            return Ok(());
        }
        if entry
            .views
            .iter()
            .take(entry.views.len() - 1)
            .any(|(_, v)| v.has_cold_lease())
        {
            return Ok(());
        }
        let view = &entry.views.back().unwrap().1;
        if !view.has_cold_lease() {
            continue;
        }
        if Arc::strong_count(view) != 1
            || Arc::weak_count(view) != 0
            || !view.exclusive_cold_lease()
        {
            return Ok(());
        }
        let committed = crate::live_index::load_map(db, &format!("view:{key}"))?;
        if committed.get("id").and_then(Value::as_str)
            != Some(&view.manifest.snapshot_ref.snapshot_id)
        {
            return Ok(());
        }
        for source in &view.manifest.sources {
            let epoch = crate::live_index::load_map(db, &format!("epoch:{}", source.source.id))?;
            if committed
                .get("epochs")
                .and_then(|e| e.get(&source.source.id))
                != Some(&Value::Object(epoch))
            {
                return Ok(());
            }
        }
    }
    let prices: Vec<_> = entries
        .values_mut()
        .map(|entry| {
            let view = &mut entry.views.back_mut().unwrap().1;
            if !view.has_cold_lease() {
                None
            } else {
                Some(
                    Arc::get_mut(view)
                        .expect("exclusive view checked")
                        .release_cold_lease(),
                )
            }
        })
        .collect();
    let timeout: i64 = db.query_row("PRAGMA busy_timeout", [], |row| row.get(0))?;
    db.busy_timeout(Duration::ZERO)?;
    let checkpoint = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
    let restore_timeout = db.busy_timeout(Duration::from_millis(timeout.max(0) as u64));
    for ((key, entry), prices) in entries.iter_mut().zip(prices) {
        if let Some(prices) = prices {
            Arc::get_mut(&mut entry.views.back_mut().unwrap().1)
                .expect("state remains locked")
                .reattach_cold_lease(db, key, prices)?;
        }
    }
    checkpoint?;
    restore_timeout?;
    Ok(())
}
pub(super) struct Control {
    settings: Mutex<()>,
    pub resident: AtomicBool,
    pub paused: AtomicBool,
    pub stopping: AtomicBool,
    pub changed: AtomicBool,
    pub watcher_ok: AtomicBool,
    pub cache_bytes: AtomicU64,
    pub instance: String,
    pub runtime: crate::service_dto::RuntimeIdentity,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    version: u32,
    paused: bool,
}
fn settings_path() -> Result<PathBuf> {
    Ok(crate::storage::data_home()?.join("user-v1/service.json"))
}
pub(crate) fn offline(request: Request) -> Result<Response> {
    Ok(Response {
        output_version: 1,
        action: request.action,
        running: false,
        resident: false,
        paused: paused()?,
        stopping: false,
        instance_id: None,
        runtime: None,
        auto_start: false,
        controls_available: true,
        preparing: None,
        ready: None,
        checked_at: None,
        revision: None,
        error_code: None,
        watcher: "unavailable".into(),
        parser_cache_bytes: 0,
        parser_cache_budget_bytes: PARSER_CACHE_BUDGET,
    })
}
pub(super) fn paused() -> Result<bool> {
    let path = settings_path()?;
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 4096 => {
            use std::io::Read;
            let mut bytes = Vec::new();
            fs::File::open(path)?.take(4097).read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 4096, "Service settings exceed budget");
            let value: Settings = serde_json::from_slice(&bytes)?;
            if value.version != 1 {
                return Err(operation_error(
                    "UNSUPPORTED_VERSION",
                    "Unsupported service settings",
                ));
            }
            Ok(value.paused)
        }
        Ok(_) => Err(operation_error(
            "RESOURCE_LIMIT",
            "Service settings exceed budget",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
impl Control {
    pub fn new(resident: bool) -> Result<Self> {
        let paused = paused()?;
        let executable = std::env::current_exe()?;
        let meta = fs::metadata(&executable)?;
        let runtime = crate::service_dto::RuntimeIdentity {
            binary: executable.to_string_lossy().into_owned(),
            bytes: meta.len(),
            modified_ns: meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
                .to_string(),
            context_key: crate::hash(serde_json::to_vec(&crate::host_environment::context()?)?),
        };
        Ok(Self {
            settings: Mutex::new(()),
            resident: AtomicBool::new(resident),
            paused: AtomicBool::new(paused),
            stopping: AtomicBool::new(false),
            changed: AtomicBool::new(false),
            watcher_ok: AtomicBool::new(false),
            cache_bytes: AtomicU64::new(0),
            instance: uuid::Uuid::new_v4().to_string(),
            runtime,
        })
    }
}
pub(super) fn prepare(shared: &Shared, jobs: &mpsc::SyncSender<Job>) -> Result<()> {
    let roots = Vec::new();
    let key = source_key(&roots);
    let mut entries = shared.0.lock().unwrap();
    let entry = entries
        .entry(key.clone())
        .or_insert_with(|| Entry::new(roots));
    if entry.paused {
        entry.restoring = true;
        if jobs
            .try_send(Job {
                key,
                restore_only: true,
            })
            .is_err()
        {
            entry.restoring = false;
            return Err(operation_error(
                "UPDATE_BUSY",
                "Unable to queue committed data restoration",
            ));
        }
        return Ok(());
    }
    scheduling::request_work(entry, &key, false, false, jobs)?;
    Ok(())
}
pub(super) fn handle(
    request: Request,
    control: &Control,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
) -> Result<Response> {
    let _settings = control
        .settings
        .lock()
        .map_err(|_| operation_error("CORE_UNAVAILABLE", "Service controls unavailable"))?;
    if control.stopping.load(Ordering::Relaxed)
        && !matches!(request.action, Action::Status | Action::Stop)
    {
        return Err(operation_error(
            "SERVICE_STOPPING",
            "Service is stopping at a transaction boundary",
        ));
    }
    match request.action {
        Action::Start | Action::Resume => {
            paused()?;
            let settings = Settings {
                version: 1,
                paused: false,
            };
            crate::storage::atomic_write(&settings_path()?, &serde_json::to_vec(&settings)?)?;
            control.paused.store(false, Ordering::Relaxed);
            control.resident.store(true, Ordering::Relaxed);
            for entry in shared.0.lock().unwrap().values_mut() {
                entry.paused = false;
            }
            prepare(shared, jobs)?;
        }
        Action::Pause => {
            paused()?;
            crate::storage::atomic_write(
                &settings_path()?,
                &serde_json::to_vec(&Settings {
                    version: 1,
                    paused: true,
                })?,
            )?;
            control.paused.store(true, Ordering::Relaxed);
        }
        Action::Stop => {
            control.stopping.store(true, Ordering::Relaxed);
        }
        Action::Enable | Action::Disable => {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Service registration belongs to the local host",
            ));
        }
        Action::Status => {}
    }
    let paused = control.paused.load(Ordering::Relaxed);
    for entry in shared.0.lock().unwrap().values_mut() {
        entry.paused = paused;
    }
    let entries = shared.0.lock().unwrap();
    let entry = entries.get(&source_key(&[]));
    let view = entry.and_then(|e| e.views.back()).map(|(_, v)| v);
    Ok(Response {
        output_version: 1,
        action: request.action,
        running: true,
        resident: control.resident.load(Ordering::Relaxed),
        paused: control.paused.load(Ordering::Relaxed),
        stopping: control.stopping.load(Ordering::Relaxed),
        instance_id: Some(control.instance.clone()),
        runtime: Some(control.runtime.clone()),
        auto_start: false,
        controls_available: true,
        preparing: entry.map(|e| e.syncing || e.restoring),
        ready: entry.map(|_| {
            view.is_some_and(|v| {
                !preview::is_initial(v)
                    && !collection::restoring_projects(v)
                    && !v.expired_detail_lease()
            })
        }),
        checked_at: entry.and_then(|e| e.checked.clone()),
        revision: view.map(|v| v.manifest.snapshot_ref.snapshot_id.clone()),
        error_code: entry.and_then(|e| e.error_code.map(str::to_owned)),
        watcher: if control.watcher_ok.load(Ordering::Relaxed) {
            "native"
        } else {
            "periodic"
        }
        .into(),
        parser_cache_bytes: control.cache_bytes.load(Ordering::Relaxed),
        parser_cache_budget_bytes: PARSER_CACHE_BUDGET,
    })
}

pub(super) struct Watch {
    watcher: Option<notify::RecommendedWatcher>,
    roots: std::collections::BTreeSet<PathBuf>,
    keys: std::collections::BTreeSet<String>,
    updated: Instant,
}
impl Watch {
    pub fn new(control: Arc<Control>, jobs: mpsc::SyncSender<Job>) -> Self {
        let callback_control = Arc::clone(&control);
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            // Access events caused by our own reads must not trigger another scan.
            if event
                .as_ref()
                .is_ok_and(|e| matches!(e.kind, notify::EventKind::Access(_)))
            {
                return;
            }
            if event.is_err() {
                callback_control.watcher_ok.store(false, Ordering::Relaxed);
            }
            if callback_control.changed.swap(true, Ordering::Relaxed) {
                return;
            }
            let _ = jobs.try_send(Job {
                key: String::new(),
                restore_only: false,
            });
        })
        .ok();
        Self {
            watcher,
            roots: Default::default(),
            keys: Default::default(),
            updated: Instant::now()
                .checked_sub(Duration::from_secs(300))
                .unwrap_or_else(Instant::now),
        }
    }
    pub fn update(&mut self, shared: &Shared, control: &Control, force: bool) {
        use notify::Watcher;
        let keys: std::collections::BTreeSet<_> =
            shared.0.lock().unwrap().keys().cloned().collect();
        if !force && keys == self.keys && self.updated.elapsed() < Duration::from_secs(300) {
            return;
        }
        self.keys = keys;
        self.updated = Instant::now();
        let roots: std::collections::BTreeSet<_> = shared
            .0
            .lock()
            .unwrap()
            .values()
            .flat_map(|e| {
                sources(&e.roots)
                    .sources
                    .into_iter()
                    .map(|s| PathBuf::from(s.root))
            })
            .flat_map(|root| [root.join("sessions"), root.join("archived_sessions"), root])
            .collect();
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        let mut complete = true;
        for root in self.roots.difference(&roots) {
            let _ = watcher.unwatch(root);
        }
        self.roots.retain(|root| roots.contains(root));
        for root in roots {
            if self.roots.contains(&root) && root.exists() {
                continue;
            }
            // A missing root remains retryable through the periodic discovery pass.
            if watcher
                .watch(
                    &root,
                    if root
                        .file_name()
                        .is_some_and(|n| n == "sessions" || n == "archived_sessions")
                    {
                        notify::RecursiveMode::Recursive
                    } else {
                        notify::RecursiveMode::NonRecursive
                    },
                )
                .is_ok()
            {
                self.roots.insert(root);
            } else {
                complete = false;
            }
        }
        control.watcher_ok.store(complete, Ordering::Relaxed);
    }
}

pub(super) fn publish_endpoint(control: &Control) -> Result<()> {
    let Some(path) = crate::host_environment::get("WOMBAT_ENDPOINT_FILE") else {
        return Ok(());
    };
    crate::storage::atomic_write(
        &PathBuf::from(path),
        &serde_json::to_vec(&json!({
            "endpointVersion":1,"protocolVersion":2,"socket":socket_path()?,"instanceId":control.instance,"pid":std::process::id(),
            "runtime":control.runtime,"context":crate::host_environment::context()?,
        }))?,
    )
}
