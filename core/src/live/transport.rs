//! Local service transport, bounded requests and connection lifecycle.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimingMessage {
    timing: crate::timing_dto::Request,
}

pub fn serve() -> Result<()> {
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
    let shared: Shared = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel::<Job>(32);
    let worker_state = Arc::clone(&shared);
    let worker = std::thread::spawn(move || -> Result<()> {
        let mut lifecycle = scheduling::WorkerLifecycle::new(Arc::clone(&worker_state));
        let mut db = crate::live_index::open(&root.join("index.sqlite")).ok();
        let mut caches = BTreeMap::new();
        loop {
            let job = match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(job) => Some(job),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => None,
            };
            if let Some(job) = &job
                && job.restore_only
            {
                let roots = worker_state.0.lock().unwrap()[&job.key].roots.clone();
                let outcome = match db.as_ref() {
                    Some(db) => restore_into(db, &job.key, &roots, &worker_state),
                    None => Err(operation_error("INDEX_UNAVAILABLE", "索引暂不可读取")),
                };
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
            let work: Vec<_> = {
                let entries = worker_state.0.lock().unwrap();
                entries
                    .iter()
                    .filter(|(key, e)| {
                        job.as_ref().is_some_and(|j| &j.key == *key)
                            || e.pending.is_none()
                                && e.following
                                && e.touched.elapsed() < Duration::from_secs(15)
                                && scheduling::auto_due(e)
                    })
                    .map(|(key, e)| {
                        (
                            key.clone(),
                            e.roots.clone(),
                            job.as_ref().is_some_and(|j| j.key == *key),
                        )
                    })
                    .collect()
            };
            for (key, roots, queued) in work {
                let work = {
                    let mut entries = worker_state.0.lock().unwrap();
                    scheduling::begin(entries.get_mut(&key).unwrap(), queued)
                };
                let Some(work) = work else { continue };
                let verify = work.verify;
                if db.is_none() {
                    match crate::live_index::open(&root.join("index.sqlite")) {
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
                let needs_restore = {
                    let mut entries = worker_state.0.lock().unwrap();
                    let entry = entries.get_mut(&key).unwrap();
                    entry.syncing = true;
                    entry.views.is_empty()
                };
                if needs_restore {
                    if let Ok(Some(view)) = preview::prepare(&key, &roots) {
                        worker_state
                            .0
                            .lock()
                            .unwrap()
                            .get_mut(&key)
                            .unwrap()
                            .views
                            .push_back((Instant::now(), view));
                        worker_state.1.notify_all();
                    }
                    if let Err(error) = restore_into(db, &key, &roots, &worker_state) {
                        // Unsupported/corrupt committed facts must not silently fall back to source collection.
                        let mut entries = worker_state.0.lock().unwrap();
                        let entry = entries.get_mut(&key).unwrap();
                        entry.error_code = Some(crate::live_index::failure_code(&error));
                        entry.error = Some(error.to_string());
                        scheduling::finish(entry, work);
                        worker_state.1.notify_all();
                        continue;
                    }
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
                let outcome = sync(db, &key, &roots, verify, prior_view.as_deref(), &mut caches);
                if outcome.is_err() {
                    caches.clear();
                }
                let mut entries = worker_state.0.lock().unwrap();
                let entry = entries.get_mut(&key).unwrap();
                entry.attempt += 1;
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
            }
        }
        if let Some(db) = db {
            db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        }
        lifecycle.completed();
        Ok(())
    });
    let mut last_client = Instant::now();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    loop {
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
                let jobs = jobs.clone();
                std::thread::spawn(move || {
                    let result = (|| -> Result<Vec<u8>> {
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_secs(12)))?;
                        stream.set_write_timeout(Some(Duration::from_secs(12)))?;
                        let mut input = String::new();
                        use std::io::Read;
                        BufReader::new(&stream)
                            .take(1024 * 1024 + 1)
                            .read_line(&mut input)?;
                        if input.len() > 1024 * 1024 {
                            return Err(operation_error("RESOURCE_LIMIT", "请求过大"));
                        }
                        #[derive(Deserialize)]
                        #[serde(deny_unknown_fields)]
                        struct ConfigMessage {
                            config: crate::config_dto::Request,
                            #[serde(default, rename = "nativeHooks")]
                            native_hooks: Option<crate::config::hooks::Capture>,
                        }
                        let value: Value = serde_json::from_str(&input)
                            .map_err(|_| operation_error("INVALID_ARGUMENT", "查询参数无效"))?;
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
                        if value.get("handoff").is_some() {
                            #[derive(Deserialize)]
                            #[serde(deny_unknown_fields)]
                            struct HandoffMessage {
                                handoff: crate::handoff_dto::Request,
                                #[serde(default, rename = "nativeHooks")]
                                native_hooks: Option<crate::config::hooks::Capture>,
                            }
                            let r: HandoffMessage =
                                serde_json::from_value(value).map_err(|_| {
                                    operation_error("INVALID_ARGUMENT", "Invalid handoff request")
                                })?;
                            let mut cfg = crate::config_dto::Request {
                                roots: r.handoff.roots.clone(),
                                project_roots: r.handoff.project_roots.clone(),
                                read_view: r.handoff.read_view.clone(),
                                ..Default::default()
                            };
                            if r.handoff.action == crate::handoff_dto::Action::Send {
                                if cfg.read_view.is_some() {
                                    config_query(cfg.clone(), None, &state, &jobs, &configs)?;
                                }
                                cfg.read_view = None;
                            }
                            let response = config_query(
                                cfg,
                                r.native_hooks.as_ref(),
                                &state,
                                &jobs,
                                &configs,
                            )?;
                            let id = response.read_view.unwrap();
                            let view = configs.lock().unwrap().get(&id)?;
                            return Ok(encode_success(&crate::handoff::prepare(
                                r.handoff, id, &view,
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
                            if r.optimize.action == crate::optimize_dto::Action::Recheck {
                                if cfg.read_view.is_some() {
                                    config_query(cfg.clone(), None, &state, &jobs, &configs)?;
                                }
                                cfg.read_view = None;
                            }
                            let response = config_query(
                                cfg,
                                r.native_hooks.as_ref(),
                                &state,
                                &jobs,
                                &configs,
                            )?;
                            let id = response.read_view.unwrap();
                            let view = configs.lock().unwrap().get(&id)?;
                            return Ok(encode_success(&crate::optimize::execute(
                                r.optimize, id, &view,
                            )?)?);
                        }
                        if value.get("config").is_some() {
                            let request: ConfigMessage = serde_json::from_value(value)
                                .map_err(|_| operation_error("INVALID_ARGUMENT", "配置参数无效"))?;
                            return Ok(encode_success(&config_query(
                                request.config,
                                request.native_hooks.as_ref(),
                                &state,
                                &jobs,
                                &configs,
                            )?)?);
                        }
                        let request: Request = serde_json::from_value(value)
                            .map_err(|_| operation_error("INVALID_ARGUMENT", "实时查询参数无效"))?;
                        Ok(encode_success(&query(request, &state, &jobs, &configs)?)?)
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
                if last_client.elapsed() > Duration::from_secs(15)
                    && active.load(std::sync::atomic::Ordering::Relaxed) == 0
                    && !configs.lock().unwrap().has_views()
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
    drop(jobs);
    let _ = worker.join();
    #[cfg(unix)]
    let _ = fs::remove_file(socket);
    Ok(())
}

fn restore_into(
    db: &rusqlite::Connection,
    key: &str,
    roots: &[String],
    shared: &Shared,
) -> Result<Option<Arc<Snapshot>>> {
    restore_projects(
        db,
        key,
        roots,
        || shared.0.lock().unwrap()[key].preferred_project.clone(),
        |view| {
            let mut entries = shared.0.lock().unwrap();
            let entry = entries.get_mut(key).unwrap();
            if view.project_loads.is_empty() {
                entry.restoring = false;
            }
            entry.views.push_back((Instant::now(), view));
            while entry.views.len() > 8 {
                entry.views.pop_front();
            }
            shared.1.notify_all();
        },
    )
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn timing_envelope_rejects_other_product_inputs_and_hooks() {
        let request = json!({"action":"capabilities"});
        assert!(serde_json::from_value::<TimingMessage>(json!({"timing":request})).is_ok());
        for field in ["query", "config", "optimize", "nativeHooks", "handoff"] {
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

// Encode typed output directly into its protocol envelope. Constructing a JSON
// value tree first duplicates every ranking field and string on the hot path.
fn encode_success(value: &impl serde::Serialize) -> serde_json::Result<Vec<u8>> {
    let mut bytes = br#"{"ok":true,"value":"#.to_vec();
    serde_json::to_writer(&mut bytes, value)?;
    bytes.push(b'}');
    Ok(bytes)
}
