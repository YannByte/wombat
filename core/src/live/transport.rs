//! Local service transport, bounded requests and connection lifecycle.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimingMessage {
    timing: crate::timing_dto::Request,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationMessage {
    conversation: crate::conversation_dto::Request,
}

pub fn serve() -> Result<()> {
    run(false)
}
pub fn serve_resident() -> Result<()> {
    run(true)
}
fn run(resident: bool) -> Result<()> {
    #[cfg(windows)]
    use crate::live_windows::Listener as UnixListener;
    use std::io::{BufRead, BufReader, Write};
    #[cfg(unix)]
    use std::os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::UnixListener,
    };
    let root = directory()?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600);
    let lock = options.open(root.join("service-v2.lock"))?;
    if lock.try_lock().is_err() {
        if resident {
            return Err(operation_error(
                "SERVICE_ALREADY_RUNNING",
                "Another service owns this data directory",
            ));
        }
        return Ok(());
    }
    let socket = socket_path()?;
    #[cfg(unix)]
    match fs::remove_file(&socket) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let listener = UnixListener::bind(&socket)?;
    #[cfg(unix)]
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let configs = Arc::new(Mutex::new(crate::config::Store::default()));
    let artifacts = Arc::new(Mutex::new(crate::artifacts::Store::default()));
    let shared: Shared = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel::<Job>(32);
    let control = Arc::new(service::Control::new(resident)?);
    service::publish_endpoint(&control)?;
    let worker_control = Arc::clone(&control);
    let worker_state = Arc::clone(&shared);
    let worker_configs = Arc::clone(&configs);
    let worker = std::thread::spawn(move || -> Result<Option<rusqlite::Connection>> {
        let mut lifecycle = scheduling::WorkerLifecycle::new(Arc::clone(&worker_state));
        // Status, startup and controls do not open or restore the derived index.
        let mut db = None;
        let mut caches = BTreeMap::new();
        let mut watch: Option<service::Watch> = None;
        let mut last_checkpoint = Instant::now();
        loop {
            if worker_control
                .stopping
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                break;
            }
            let job = match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(job) => Some(job),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => None,
            };
            if job.is_some() {
                watch
                    .get_or_insert_with(|| {
                        service::Watch::new(Arc::clone(&worker_control), Arc::clone(&worker_state))
                    })
                    .update(&worker_state, &worker_control);
            }
            if let Some(job) = &job
                && job.restore_only
            {
                let roots = worker_state.0.lock().unwrap()[&job.key].roots.clone();
                let outcome = (|| {
                    if db.is_none() {
                        db = Some(crate::live_index::open(&generation::index_path(&root)?)?);
                    }
                    restore_into(db.as_ref().unwrap(), &job.key, &roots, &worker_state)
                })();
                let mut entries = worker_state.0.lock().unwrap();
                let entry = entries.get_mut(&job.key).unwrap();
                entry.restoring = false;
                if let Err(error) = outcome {
                    entry.error_code = Some(crate::live_index::failure_code(&error));
                    entry.error = Some(error.to_string());
                }
                worker_state.1.notify_all();
                continue;
            }
            // Only caller-enqueued jobs run. Notifications and elapsed time are
            // freshness evidence for the next query, never a reason to scan now.
            if let Some(Job { key, .. }) = job {
                let roots = worker_state.0.lock().unwrap()[&key].roots.clone();
                if source_key(&roots) != key {
                    let mut entries = worker_state.0.lock().unwrap();
                    let entry = entries.get_mut(&key).unwrap();
                    if let Some(work) = scheduling::begin(entry) {
                        scheduling::finish(entry, work);
                    }
                    entry.error = Some("Source scope changed; reacquire the default view".into());
                    entry.error_code = Some("SOURCE_SCOPE_CHANGED");
                    worker_state.1.notify_all();
                    continue;
                }
                let work = {
                    let mut entries = worker_state.0.lock().unwrap();
                    scheduling::begin(entries.get_mut(&key).unwrap())
                };
                let Some(work) = work else { continue };
                worker_state
                    .0
                    .lock()
                    .unwrap()
                    .get_mut(&key)
                    .unwrap()
                    .changed = false;
                if worker_control
                    .paused
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    let mut entries = worker_state.0.lock().unwrap();
                    scheduling::finish(entries.get_mut(&key).unwrap(), work);
                    worker_state.1.notify_all();
                    continue;
                }
                let verify = work.verify;
                if work.rebuild {
                    let outcome = resources::index_batch(|| {
                        let (next, view) = generation::rebuild(&root, &key, &roots)?;
                        drop(view);
                        Ok(next)
                    })
                    .and_then(|next| {
                        let view = restore_view(&next, &key, &roots)?.ok_or_else(|| {
                            operation_error("INDEX_UNAVAILABLE", "Rebuilt header unavailable")
                        })?;
                        Ok((next, view))
                    });
                    let mut entries = worker_state.0.lock().unwrap();
                    let entry = entries.get_mut(&key).unwrap();
                    scheduling::finish(entry, work);
                    entry.last_sync = Instant::now();
                    match outcome {
                        Ok((next, view)) => {
                            db = Some(next);
                            caches.clear();
                            entry.views.push_back((Instant::now(), view));
                            entry.error = None;
                            entry.error_code = None;
                            entry.checked = Some(chrono::Utc::now().to_rfc3339());
                            while entry.views.len() > 8 {
                                entry.views.pop_front();
                            }
                            // Other source sets must reacquire locators in the new database.
                            for (other_key, other) in entries.iter_mut() {
                                if other_key != &key {
                                    other.views.clear();
                                    if other.pending.is_none() {
                                        other.requested = 0;
                                        other.completed = 0;
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            entry.error_code = Some(crate::live_index::failure_code(&error));
                            entry.error = Some(error.to_string());
                        }
                    }
                    worker_state.1.notify_all();
                    continue;
                }
                if db.is_none() {
                    match generation::index_path(&root)
                        .and_then(|path| crate::live_index::open(&path))
                    {
                        Ok(opened) => db = Some(opened),
                        Err(error) => {
                            let mut entries = worker_state.0.lock().unwrap();
                            let entry = entries.get_mut(&key).unwrap();
                            entry.error_code = Some(crate::live_index::failure_code(&error));
                            entry.error = Some(error.to_string());
                            entry.last_sync = Instant::now();
                            entry.checked = Some(chrono::Utc::now().to_rfc3339());
                            scheduling::finish(entry, work);
                            worker_state.1.notify_all();
                            continue;
                        }
                    }
                }
                let db = db.as_mut().unwrap();
                let needs_restore = worker_state.0.lock().unwrap()[&key].views.is_empty();
                if needs_restore && let Err(error) = restore_into(db, &key, &roots, &worker_state) {
                    // Unsupported/corrupt committed facts must not silently fall back to source collection.
                    let mut entries = worker_state.0.lock().unwrap();
                    let entry = entries.get_mut(&key).unwrap();
                    entry.error_code = Some(crate::live_index::failure_code(&error));
                    entry.error = Some(error.to_string());
                    scheduling::finish(entry, work);
                    worker_state.1.notify_all();
                    continue;
                }
                let prior_view = worker_state.0.lock().unwrap()[&key]
                    .views
                    .iter()
                    .rev()
                    .find(|(_, v)| !preview::is_initial(v) && !collection::restoring_projects(v))
                    .map(|(_, v)| Arc::clone(v));
                if prior_view.is_none()
                    && worker_state.0.lock().unwrap()[&key].views.is_empty()
                    && let Ok(Some(view)) = preview::prepare(&key, &roots)
                {
                    let mut entries = worker_state.0.lock().unwrap();
                    let entry = entries.get_mut(&key).unwrap();
                    if entry.views.is_empty() {
                        entry.views.push_back((Instant::now(), view));
                    }
                    worker_state.1.notify_all();
                }
                let (mut outcome, compact, mut resident_bytes, evicted) =
                    resources::index_batch(|| {
                        let mut outcome = collection::sync_with_policy(
                            db,
                            &key,
                            &roots,
                            verify,
                            prior_view.as_deref(),
                            &mut caches,
                            !work.include_history,
                        );
                        let compact = outcome.as_ref().is_ok_and(|view| view.is_some());
                        if compact {
                            outcome = Ok(None);
                        }
                        let mut evicted = outcome.is_err();
                        if outcome.is_err() {
                            caches.clear();
                        }
                        let (resident_bytes, trimmed) =
                            resources::trim_parser_caches(db, &mut caches)?;
                        evicted |= trimmed;
                        if evicted {
                            db.flush_prepared_statement_cache();
                            db.execute_batch("PRAGMA shrink_memory")?;
                            resources::release_unused();
                        }
                        Ok((outcome, compact, resident_bytes, evicted))
                    })?;
                if compact {
                    outcome = restore_view(db, &key, &roots);
                }
                if outcome.is_err() {
                    caches.clear();
                    resident_bytes = 0;
                }
                worker_control
                    .cache_bytes
                    .store(resident_bytes, std::sync::atomic::Ordering::Relaxed);
                let mut entries = worker_state.0.lock().unwrap();
                let entry = entries.get_mut(&key).unwrap();
                entry.last_sync = Instant::now();
                scheduling::finish(entry, work);
                match outcome {
                    Ok(view) => {
                        if let Some(view) = view {
                            entry.views.push_back((Instant::now(), view));
                        }
                        entry.checked = Some(chrono::Utc::now().to_rfc3339());
                        entry.error = None;
                        entry.error_code = None;
                    }
                    Err(e) => {
                        entry.error_code = Some(crate::live_index::failure_code(&e));
                        entry.error = Some(format!("{e:#}"));
                    }
                }
                while entry.views.len() > 1
                    && entry.views.front().is_some_and(|(t, _)| {
                        t.elapsed() > Duration::from_secs(600) || entry.views.len() > 8
                    })
                {
                    entry.views.pop_front();
                }
                worker_state.1.notify_all();
                drop(entries);
                drop(prior_view);
                if evicted || compact {
                    resources::release_unused();
                }
            }
            // Expire leases even when no new view is published. A permanent
            // service cannot rely on another query or process exit for cleanup.
            {
                let mut entries = worker_state.0.lock().unwrap();
                for entry in entries.values_mut() {
                    if entry.touched.elapsed() > Duration::from_secs(60) {
                        for (_, view) in &entry.views {
                            view.release_detail_cache();
                        }
                    }
                    while entry.views.len() > 1
                        && entry
                            .views
                            .front()
                            .is_some_and(|(at, _)| at.elapsed() > Duration::from_secs(600))
                    {
                        entry.views.pop_front();
                    }
                }
                entries.retain(|_, entry| {
                    entry.roots.is_empty()
                        || entry.syncing
                        || entry.restoring
                        || entry.touched.elapsed() < Duration::from_secs(600)
                        || entry
                            .views
                            .iter()
                            .any(|(_, view)| Arc::strong_count(view) > 1)
                });
            }
            if last_checkpoint.elapsed() >= Duration::from_secs(30) {
                worker_configs.lock().unwrap().has_views();
                if let Some(db) = &db {
                    if service::wal_pressure(db) {
                        worker_configs.lock().unwrap().evict_unshared_snapshots();
                    }
                    db.execute_batch("PRAGMA wal_checkpoint(PASSIVE)")?;
                    service::reclaim_wal(db, &worker_state)?;
                }
                last_checkpoint = Instant::now();
            }
        }
        lifecycle.completed();
        Ok(db)
    });
    let mut last_client = Instant::now();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    loop {
        // Keep status available while an in-flight writer reaches its safe
        // transaction boundary. Stop accepting only after that worker exits.
        if worker.is_finished() && active.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                last_client = Instant::now();
                if active.load(std::sync::atomic::Ordering::Relaxed) >= 8 {
                    continue;
                }
                active.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let active = Arc::clone(&active);
                let state = Arc::clone(&shared);
                let configs = Arc::clone(&configs);
                let artifacts = Arc::clone(&artifacts);
                let jobs = jobs.clone();
                let control = Arc::clone(&control);
                std::thread::spawn(move || {
                    resources::interactive_request();
                    let result = (|| -> Result<Vec<u8>> {
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_secs(12)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(12)))?;
                        let mut input = String::new();
                        use std::io::Read;
                        let mut reader = BufReader::new(&stream);
                        reader
                            .by_ref()
                            .take(1024 * 1024 + 1)
                            .read_line(&mut input)?;
                        let buffered_cancel = reader.buffer().first() == Some(&0);
                        if input.len() > 1024 * 1024 {
                            return Err(operation_error("RESOURCE_LIMIT", "请求过大"));
                        }
                        #[derive(Deserialize)]
                        #[serde(deny_unknown_fields)]
                        struct ConfigMessage {
                            config: crate::config_dto::Request,
                            #[serde(default, rename = "nativeHooks")]
                            native_hooks: Option<crate::config::hooks::Capture>,
                            #[serde(default, rename = "nativeSkills")]
                            native_skills: Option<crate::config::review::Capture>,
                        }
                        let mut value: Value = serde_json::from_str(&input)
                            .map_err(|_| operation_error("INVALID_ARGUMENT", "查询参数无效"))?;
                        if let Some(instance) = value
                            .as_object_mut()
                            .and_then(|v| v.remove("serviceInstanceId"))
                            && instance.as_str() != Some(control.instance.as_str())
                        {
                            return Err(operation_error(
                                "SERVICE_CHANGED",
                                "Service instance changed; reconnect",
                            ));
                        }
                        if value.get("service").is_some() {
                            #[derive(Deserialize)]
                            #[serde(deny_unknown_fields)]
                            struct ServiceMessage {
                                service: crate::service_dto::Request,
                            }
                            let request: ServiceMessage =
                                serde_json::from_value(value).map_err(|_| {
                                    operation_error("INVALID_ARGUMENT", "Invalid service request")
                                })?;
                            return Ok(encode_success(&service::handle(
                                request.service,
                                &control,
                                &state,
                            )?)?);
                        }
                        if control.stopping.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(operation_error(
                                "SERVICE_STOPPING",
                                "Service is stopping at a transaction boundary",
                            ));
                        }
                        if value.get("artifacts").is_some() {
                            #[derive(Deserialize)]
                            #[serde(deny_unknown_fields)]
                            struct ArtifactMessage {
                                artifacts: crate::artifacts_dto::Request,
                            }
                            let request: ArtifactMessage =
                                serde_json::from_value(value).map_err(|_| {
                                    operation_error("INVALID_ARGUMENT", "Invalid artifact request")
                                })?;
                            let cancellation = PeerCancellation::new(buffered_cancel);
                            #[cfg(unix)]
                            let probe =
                                || cancellation.observe(Instant::now(), || peer_closed(&stream));
                            #[cfg(windows)]
                            let peer_probe = stream.peer_probe();
                            #[cfg(windows)]
                            let probe = || cancellation.observe(Instant::now(), &peer_probe);
                            return Ok(encode_success(&crate::artifacts::execute(
                                request.artifacts,
                                &artifacts,
                                &probe,
                            )?)?);
                        }
                        if value.get("monitor").is_some() {
                            #[derive(Deserialize)]
                            #[serde(deny_unknown_fields)]
                            struct MonitorMessage {
                                monitor: crate::monitor::Request,
                            }
                            let request: MonitorMessage =
                                serde_json::from_value(value).map_err(|_| {
                                    operation_error("INVALID_ARGUMENT", "Invalid monitor request")
                                })?;
                            return Ok(encode_success(&query::monitor_query(
                                request.monitor,
                                &state,
                                &jobs,
                                &configs,
                            )?)?);
                        }
                        if value.get("timing").is_some() {
                            let request: TimingMessage =
                                serde_json::from_value(value).map_err(|_| {
                                    operation_error("INVALID_ARGUMENT", "Invalid timing request")
                                })?;
                            let cancelled = std::sync::atomic::AtomicBool::new(false);
                            return Ok(encode_success(&timing_query(
                                request.timing,
                                &state,
                                &jobs,
                                &configs,
                                &cancelled,
                            )?)?);
                        }
                        if value.get("conversation").is_some() {
                            let request: ConversationMessage = serde_json::from_value(value)
                                .map_err(|_| {
                                    operation_error(
                                        "INVALID_ARGUMENT",
                                        "Invalid conversation request",
                                    )
                                })?;
                            return Ok(encode_success(&query::conversation_query(
                                request.conversation,
                                &state,
                                &jobs,
                                &configs,
                                &std::sync::atomic::AtomicBool::new(false),
                            )?)?);
                        }
                        if value.get("optimize").is_some() {
                            #[derive(Deserialize)]
                            #[serde(deny_unknown_fields)]
                            struct OptimizeMessage {
                                optimize: crate::optimize_dto::Request,
                                #[serde(default, rename = "nativeHooks")]
                                native_hooks: Option<crate::config::hooks::Capture>,
                            }
                            let r: OptimizeMessage = serde_json::from_value(value)
                                .map_err(|_| operation_error("INVALID_ARGUMENT", "优化参数无效"))?;
                            crate::optimize::activity::validate(&r.optimize)?;
                            if r.optimize.action == crate::optimize_dto::Action::Activity {
                                if r.native_hooks.is_some() {
                                    return Err(operation_error(
                                        "INVALID_ARGUMENT",
                                        "Activity does not consume host hooks",
                                    ));
                                }
                                let cancelled = std::sync::atomic::AtomicBool::new(false);
                                let result = timing_query(
                                    crate::optimize::activity::timing_request(&r.optimize)?,
                                    &state,
                                    &jobs,
                                    &configs,
                                    &cancelled,
                                )?;
                                return Ok(encode_success(&crate::optimize::activity::response(
                                    result,
                                )?)?);
                            }
                            if r.optimize.action == crate::optimize_dto::Action::Capabilities {
                                let mut response = crate::optimize::capabilities();
                                response.rule_parameters =
                                    crate::optimize::parameters(r.optimize.rule_overrides)?;
                                return Ok(encode_success(&response)?);
                            }
                            let mut cfg = crate::config_dto::Request {
                                roots: r.optimize.roots.clone(),
                                project_roots: r.optimize.project_roots.clone(),
                                read_view: r.optimize.read_view.clone(),
                                ..Default::default()
                            };
                            let baseline =
                                if r.optimize.action == crate::optimize_dto::Action::Recheck {
                                    let key = cfg
                                        .read_view
                                        .as_ref()
                                        .ok_or_else(|| {
                                            operation_error(
                                                "INVALID_ARGUMENT",
                                                "Recheck requires its original read view",
                                            )
                                        })?
                                        .clone();
                                    config_query(cfg.clone(), None, None, &state, &jobs, &configs)?;
                                    let baseline = configs.lock().unwrap().get(&key)?;
                                    cfg.read_view = None;
                                    Some(baseline)
                                } else {
                                    None
                                };
                            let response = config_query(
                                cfg,
                                r.native_hooks.as_ref(),
                                None,
                                &state,
                                &jobs,
                                &configs,
                            )?;
                            let id = response.read_view.unwrap();
                            let view = configs.lock().unwrap().get(&id)?;
                            return Ok(encode_success(&crate::optimize::execute(
                                r.optimize,
                                id,
                                &view,
                                baseline.as_deref(),
                            )?)?);
                        }
                        if value.get("config").is_some() {
                            let request: ConfigMessage = serde_json::from_value(value)
                                .map_err(|_| operation_error("INVALID_ARGUMENT", "配置参数无效"))?;
                            return Ok(encode_success(&config_query(
                                request.config,
                                request.native_hooks.as_ref(),
                                request.native_skills.as_ref(),
                                &state,
                                &jobs,
                                &configs,
                            )?)?);
                        }
                        let request: Request = serde_json::from_value(value)
                            .map_err(|_| operation_error("INVALID_ARGUMENT", "实时查询参数无效"))?;
                        let cancellation = PeerCancellation::new(buffered_cancel);
                        #[cfg(unix)]
                        let probe =
                            || cancellation.observe(Instant::now(), || peer_closed(&stream));
                        #[cfg(windows)]
                        let peer_probe = stream.peer_probe();
                        #[cfg(windows)]
                        let probe = || cancellation.observe(Instant::now(), &peer_probe);
                        Ok(encode_success(&query::query_with_probe(
                            request,
                            &state,
                            &jobs,
                            &configs,
                            Some(&probe),
                        )?)?)
                    })();
                    let response = match result {
                        Ok(bytes) => Ok(bytes),
                        Err(e) => serde_json::to_vec(
                            &json!({"ok":false,"code":e.downcast_ref::<crate::dto::OperationError>().map_or("CORE_ERROR",|e|e.code),"error":format!("{e:#}")}),
                        ),
                    };
                    if let Ok(bytes) = response {
                        let _ = stream.write_all(&bytes);
                    }
                    let _ = stream.write_all(b"\n");
                    drop(stream);
                    active.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Publication can take longer than the idle grace period.
                // Count idle time after active readers finish, not from accept.
                if active.load(std::sync::atomic::Ordering::Relaxed) != 0 {
                    last_client = Instant::now();
                }
                if !control.resident.load(std::sync::atomic::Ordering::Relaxed)
                    && last_client.elapsed() > Duration::from_secs(15)
                    && active.load(std::sync::atomic::Ordering::Relaxed) == 0
                    && !configs.lock().unwrap().has_views()
                    && !artifacts.lock().unwrap().has_views()
                    && !shared
                        .0
                        .lock()
                        .unwrap()
                        .values()
                        .any(|e| e.syncing || e.restoring)
                {
                    break;
                }
                #[cfg(unix)]
                wait_for_connection(&listener, 250)?;
                #[cfg(windows)]
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e.into()),
        }
    }
    // No active callers remain. Release read leases before the writer checkpoints
    // and closes, so an idle exit does not leave its own obsolete WAL pages pinned.
    *configs.lock().unwrap() = crate::config::Store::default();
    *artifacts.lock().unwrap() = crate::artifacts::Store::default();
    shared.0.lock().unwrap().clear();
    control
        .stopping
        .store(true, std::sync::atomic::Ordering::Relaxed);
    drop(jobs);
    let worker_result = worker.join();
    #[cfg(unix)]
    let _ = fs::remove_file(socket);
    let db = worker_result.map_err(|_| {
        operation_error("CORE_UNAVAILABLE", "Background indexing worker panicked")
    })??;
    if let Some(db) = db {
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    }
    Ok(())
}

// Fact loops can check cancellation very often. Probe the OS at most once per
// millisecond while preserving an already observed cancellation immediately.
struct PeerCancellation {
    state: Mutex<(Option<Instant>, bool)>,
}
impl PeerCancellation {
    fn new(cancelled: bool) -> Self {
        Self {
            state: Mutex::new((None, cancelled)),
        }
    }
    fn observe(&self, now: Instant, check: impl FnOnce() -> bool) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return true;
        };
        if !state.1
            && state
                .0
                .is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_millis(1))
        {
            state.0 = Some(now);
            state.1 = check();
        }
        state.1
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn repeated_fact_checks_coalesce_and_observed_cancellation_stays_immediate() {
        let at = Instant::now();
        let probe = PeerCancellation::new(false);
        let calls = std::cell::Cell::new(0);
        for _ in 0..10000 {
            assert!(!probe.observe(at, || {
                calls.set(calls.get() + 1);
                false
            }));
        }
        assert_eq!(calls.get(), 1);
        assert!(!probe.observe(at + Duration::from_micros(999), || panic!("too early")));
        assert!(probe.observe(at + Duration::from_millis(1), || true));
        assert!(probe.observe(at + Duration::from_millis(2), || panic!(
            "already cancelled"
        )));
        assert!(PeerCancellation::new(true).observe(at, || panic!("buffered cancellation")));
    }
}

#[cfg(unix)]
fn peer_closed(stream: &std::os::unix::net::UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    let mut descriptor = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: 0,
        revents: 0,
    };
    let mut byte = 255_u8;
    // NUL is a caller-cancellation marker after the complete JSON request.
    // EOF alone can also mean a valid write half-close.
    if unsafe {
        libc::recv(
            stream.as_raw_fd(),
            (&raw mut byte).cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    } == 1
        && byte == 0
    {
        return true;
    }
    // SAFETY: one initialized descriptor, borrowed from a live stream; no wait.
    // A caller can finish writing and still receive its response.
    (unsafe { libc::poll(&raw mut descriptor, 1, 0) > 0 })
        && descriptor.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}
#[cfg(all(test, unix))]
mod peer_tests {
    use super::*;
    #[test]
    fn disconnected_waiter_is_detected_but_write_half_close_can_receive_a_result() {
        let (server, client) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(!peer_closed(&server));
        client.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(!peer_closed(&server));
        let (server, mut client) = std::os::unix::net::UnixStream::pair().unwrap();
        use std::io::Write;
        client.write_all(&[0]).unwrap();
        assert!(peer_closed(&server));
    }
}

fn restore_into(
    db: &rusqlite::Connection,
    key: &str,
    roots: &[String],
    shared: &Shared,
) -> Result<()> {
    if let Some(view) = restore_view(db, key, roots)? {
        let mut entries = shared.0.lock().unwrap();
        let entry = entries.get_mut(key).unwrap();
        entry.restoring = false;
        entry.views.push_back((Instant::now(), view));
        while entry.views.len() > 8 {
            entry.views.pop_front();
        }
        shared.1.notify_all();
    }
    Ok(())
}

/// Readiness wakes immediately for a request; the timeout only bounds lifecycle checks.
#[cfg(unix)]
fn wait_for_connection(
    listener: &std::os::unix::net::UnixListener,
    timeout_ms: i32,
) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;
    let mut descriptor = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one initialized pollfd stays live for the call, and the borrowed
    // listener owns its descriptor throughout this bounded wait.
    let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(error);
    }
    if descriptor.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
        return Err(std::io::Error::other("local listener readiness failed"));
    }
    Ok(descriptor.revents & libc::POLLIN != 0)
}

// Encode typed output directly into its protocol envelope. Constructing a JSON
// value tree first duplicates every ranking field and string on the hot path.
fn encode_success(value: &impl serde::Serialize) -> serde_json::Result<Vec<u8>> {
    let mut bytes = br#"{"ok":true,"value":"#.to_vec();
    serde_json::to_writer(&mut bytes, value)?;
    bytes.push(b'}');
    Ok(bytes)
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn timing_envelope_rejects_other_product_inputs_and_hooks() {
        let request = json!({"action":"capabilities"});
        assert!(serde_json::from_value::<TimingMessage>(json!({"timing":request})).is_ok());
        for field in ["query", "config", "optimize", "nativeHooks"] {
            let mut value = json!({"timing":request});
            value[field] = json!({});
            assert!(serde_json::from_value::<TimingMessage>(value).is_err());
        }
        assert!(
            serde_json::from_value::<TimingMessage>(json!({
                "timing":{"action":"capabilities","verify":true}
            }))
            .is_err()
        );
    }
}

#[cfg(all(test, unix))]
mod readiness_tests {
    use super::*;
    #[test]
    fn readiness_observes_queued_connection_and_empty_timeout() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(!wait_for_connection(&listener, 0).unwrap());
        let _client = std::os::unix::net::UnixStream::connect(&path).unwrap();
        assert!(wait_for_connection(&listener, 0).unwrap());
        let _accepted = listener.accept().unwrap();
        assert!(!wait_for_connection(&listener, 0).unwrap());
    }
}
