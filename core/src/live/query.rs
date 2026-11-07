//! A single foreground request owns synchronization and all dependent reads.
use super::*;
use std::{
    fs::File,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub(super) struct Read {
    pub(super) snapshot: Arc<Snapshot>,
    pub(super) freshness: Freshness,
}
fn acquire_until(
    root: &std::path::Path,
    complete: impl Fn() -> Result<bool>,
) -> Result<Option<File>> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(root.join("request.lock"))?;
    let started = Instant::now();
    loop {
        if complete()? {
            return Ok(None);
        }
        match lock.try_lock() {
            Ok(()) => return Ok(Some(lock)),
            Err(std::fs::TryLockError::WouldBlock)
                if started.elapsed() < Duration::from_secs(600) =>
            {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(operation_error(
                    "INDEX_BUSY",
                    "Another request is updating the index; retry the same scope",
                ));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
}
fn acquire(root: &std::path::Path) -> Result<File> {
    acquire_until(root, || Ok(false))?
        .ok_or_else(|| operation_error("INDEX_BUSY", "Request lock was not acquired"))
}
pub(super) fn read(
    roots: &[String],
    mode: Mode,
    verify: bool,
    rebuild: bool,
    recent_only: bool,
) -> Result<Read> {
    let root = directory()?;
    let key = source_key(roots);
    let (db, snapshot, checked) = if rebuild {
        let (db, snapshot) = generation::rebuild(&root, &key, roots)?;
        (db, snapshot, chrono::Utc::now().to_rfc3339())
    } else {
        let mut db = crate::live_index::open(&generation::index_path(&root)?)?;
        let prior = collection::restore_view(&db, &key, roots)?;
        if mode == Mode::Cached {
            let snapshot = prior.ok_or_else(|| {
                operation_error(
                    "INDEX_UNAVAILABLE",
                    "No collected facts for this source scope; run usage --fresh",
                )
            })?;
            let checked = crate::live_index::load_map(&db, &format!("checked:{key}"))?
                .get("at")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| snapshot.manifest.snapshot_ref.created_at.clone());
            (db, snapshot, checked)
        } else {
            let snapshot = resources::index_batch(|| {
                collection::sync_with_policy(
                    &mut db,
                    &key,
                    roots,
                    verify,
                    prior.as_deref(),
                    &mut BTreeMap::new(),
                    recent_only,
                )
            })?
            .or(prior)
            .ok_or_else(|| {
                operation_error(
                    "INDEX_UNAVAILABLE",
                    "Collection produced no committed facts",
                )
            })?;
            let checked = chrono::Utc::now().to_rfc3339();
            crate::live_index::save_map(
                &db,
                &format!("checked:{key}"),
                json!({"at":checked}).as_object().unwrap(),
            )?;
            (db, snapshot, checked)
        }
    };
    let freshness = Freshness {
        publication_change: snapshot.publication_change.clone(),
        project_loads: snapshot.project_loads.clone(),
        initial_scan: false,
        status: if mode == Mode::Cached {
            "cached"
        } else {
            "current"
        }
        .into(),
        checked_at: Some(checked),
        revision: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
        error: None,
        error_code: None,
    };
    // The response owns a short read transaction; no process retains it after stdout closes.
    db.execute_batch("PRAGMA wal_checkpoint(PASSIVE)")?;
    Ok(Read {
        snapshot,
        freshness,
    })
}
pub(crate) fn cleanup_rebuild(request_id: &str) -> Result<usize> {
    let root = directory()?;
    let Some(_lock) = acquire_until(&root, || {
        Ok(!super::generation::has_pending(&root, request_id)?)
    })?
    else {
        return Ok(0);
    };
    super::generation::cleanup_pending(&root, None)
}
fn normalize_projects(roots: &mut Option<Vec<String>>, project: &mut Option<String>) -> Result<()> {
    if let Some(roots) = roots {
        for root in roots.iter_mut() {
            *root = crate::directories::canonical_project(root)?;
        }
        roots.sort_unstable();
        roots.dedup();
    }
    if let Some(project) = project {
        *project = crate::directories::canonical_project(project)?;
    }
    Ok(())
}
fn config_read(
    request: &crate::config_dto::Request,
    hooks: Option<&crate::config::hooks::Capture>,
    skills: Option<&crate::config::review::Capture>,
) -> Result<crate::config::View> {
    crate::config::validate(request)?;
    let selected = read(
        request.roots.as_deref().unwrap_or_default(),
        if request.cached_usage {
            Mode::Cached
        } else {
            Mode::Auto
        },
        false,
        false,
        false,
    );
    let (snapshot, status, error) = match selected {
        Ok(r) => (
            Some(r.snapshot.detailed_view()?.unwrap_or(r.snapshot)),
            r.freshness.status,
            None,
        ),
        Err(e) => (
            None,
            "unavailable".into(),
            Some(crate::live_index::failure_code(&e).to_owned()),
        ),
    };
    let mut inventory = crate::config::prepare_observed(request, snapshot, status, hooks, skills)?;
    if let Some(code) = error {
        inventory
            .issues
            .push(crate::config_dto::Issue { code, path: None });
    }
    Ok(inventory)
}
fn direct_report(request: &Request) -> bool {
    fn ledger(action: &usage_app_dto::Action) -> bool {
        use usage_app_dto::Action::*;
        matches!(action, Usage | Statistics | Threads)
    }
    request.mode == Mode::Auto
        && !request.verify
        && !request.rebuild
        && ledger(&request.query.action)
        && request.also.iter().all(|q| ledger(&q.action))
}

struct ObservedUsage {
    roots: Vec<String>,
    input: crate::adapters::codex::usage_input::Inventory,
    elapsed: Duration,
}

fn direct_usage(roots: &[String], recent_only: bool) -> Result<(Read, Option<ObservedUsage>)> {
    use super::usage_cache;
    let prices = crate::pricing_sync::current()?;
    let input = usage_cache::candidate(roots, recent_only);
    let current = |input: &crate::adapters::codex::usage_input::Inventory| {
        usage_cache::candidate(roots, recent_only)
            .is_some_and(|i| i.fingerprint == input.fingerprint)
    };
    let snapshot_id = |input: Option<&crate::adapters::codex::usage_input::Inventory>| {
        let data = input.map_or_else(
            || uuid::Uuid::new_v4().to_string(),
            |i| i.fingerprint.clone(),
        );
        format!(
            "source-usage:{}",
            crate::adapters::stable_id(&[&data, &prices.catalog_hash])
        )
    };
    let ready = input.as_ref().and_then(|i| {
        let collected = usage_cache::load(i)?;
        let snapshot =
            crate::usage_store::memory_usage(collected, snapshot_id(Some(i)), prices.clone())
                .ok()?;
        current(i).then_some(snapshot)
    });
    let (snapshot, observed) = if let Some(snapshot) = ready {
        (snapshot, None)
    } else {
        let start = Instant::now();
        let collected = super::usage_source::collect(roots, recent_only, &RunContext::default())?;
        let elapsed = start.elapsed();
        let input = input.filter(current);
        let snapshot =
            crate::usage_store::memory_usage(collected, snapshot_id(input.as_ref()), prices)?;
        let observed = input.map(|input| ObservedUsage {
            roots: roots.to_vec(),
            input,
            elapsed,
        });
        (snapshot, observed)
    };
    let freshness = Freshness {
        publication_change: None,
        project_loads: vec![],
        initial_scan: false,
        status: "current".into(),
        checked_at: Some(chrono::Utc::now().to_rfc3339()),
        revision: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
        error: None,
        error_code: None,
    };
    Ok((
        Read {
            snapshot: Arc::new(snapshot),
            freshness,
        },
        observed,
    ))
}

fn usage(request: Request) -> Result<Response> {
    if request.also.len() > 8 || (request.verify || request.rebuild) && request.mode != Mode::Fresh
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Invalid query batch or collection controls",
        ));
    }
    if request.query.action == usage_app_dto::Action::Refresh {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Saved-view refresh is not supported; use a usage query with mode fresh",
        ));
    }
    let mut validation = request.query.clone();
    validation.roots = None;
    crate::usage_app::validate(&validation)?;
    for q in &request.also {
        if q.roots.is_some()
            || q.snapshot_id.is_some()
            || q.action == usage_app_dto::Action::Refresh
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Additional queries use the same source scope and cannot refresh",
            ));
        }
        crate::usage_app::validate(q)?;
    }
    let cutoff = (chrono::Utc::now().date_naive() - chrono::Duration::days(30)).to_string();
    let historical = |q: &usage_app_dto::Request| {
        !matches!(
            q.action,
            usage_app_dto::Action::Usage
                | usage_app_dto::Action::Threads
                | usage_app_dto::Action::Statistics
        ) || q.scope.all_time == Some(true)
            || q.action == usage_app_dto::Action::Threads && q.scope.since.is_none()
            || q.scope.undated == Some(true)
            || q.scope.since.as_ref().is_some_and(|v| v < &cutoff)
            || q.scope.until.as_ref().is_some_and(|v| v <= &cutoff)
            || matches!(&q.comparison,Some(usage_app_dto::ComparisonRequest::Periods{baseline_since,..}) if baseline_since<&cutoff)
    };
    let roots = request.query.roots.as_deref().unwrap_or_default();
    let recent_only = !(request.include_history
        || request.verify
        || request.rebuild
        || historical(&request.query)
        || request.also.iter().any(historical));
    let (selected, observed) = if direct_report(&request) {
        direct_usage(roots, recent_only)?
    } else {
        (
            read(
                roots,
                request.mode,
                request.verify,
                request.rebuild,
                recent_only,
            )?,
            None,
        )
    };
    let execute = |mut q: usage_app_dto::Request| -> Result<usage_app_dto::Response> {
        q.roots = None;
        q.snapshot_id = None;
        let mut result = crate::usage_app::execute_snapshot(q, &selected.snapshot)?;
        result.freshness = Some(selected.freshness.clone());
        Ok(result)
    };
    let result = execute(request.query)?;
    let additional = request
        .also
        .into_iter()
        .map(execute)
        .collect::<Result<Vec<_>>>()?;
    let response = Response {
        output_version: 1,
        result,
        freshness: selected.freshness,
        additional,
    };
    if !response.additional.is_empty() && serde_json::to_vec(&response)?.len() > 2 * 1024 * 1024 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Usage batch exceeds response budget",
        ));
    }
    if let Some(observed) = observed {
        // Reuse is optional. A failed admission never changes the report.
        let _ = super::usage_cache::observed(&observed.roots, &observed.input, observed.elapsed);
    }
    Ok(response)
}
fn timing(mut r: crate::timing_dto::Request) -> Result<crate::timing_dto::Response> {
    use crate::timing_dto::Request as R;
    let (roots, mode) = match &r {
        R::Capabilities { privacy_profile } => {
            return crate::timing::capabilities(*privacy_profile);
        }
        R::Summary { roots, mode, .. } => (
            roots.clone(),
            match mode {
                crate::timing_dto::Mode::Auto => Mode::Auto,
                crate::timing_dto::Mode::Fresh => Mode::Fresh,
                crate::timing_dto::Mode::Cached => Mode::Cached,
            },
        ),
        R::Evidence { roots, .. } => (roots.clone(), Mode::Cached),
    };
    let selected = read(&roots, mode, false, false, false)?;
    match &mut r {
        R::Summary { snapshot_id, .. } => *snapshot_id = Some(selected.freshness.revision.clone()),
        R::Evidence { snapshot_id, .. } => *snapshot_id = selected.freshness.revision.clone(),
        _ => {}
    }
    let snapshot = selected
        .snapshot
        .detailed_view()?
        .unwrap_or(selected.snapshot);
    crate::timing::query_on_snapshot(
        &snapshot,
        &r,
        crate::timing_dto::QueryFreshness {
            status: selected.freshness.status,
            checked_at: selected.freshness.checked_at,
            revision: None,
            error_code: None,
        },
        &AtomicBool::new(false),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ConfigMessage {
    config: crate::config_dto::Request,
    native_hooks: Option<crate::config::hooks::Capture>,
    native_skills: Option<crate::config::review::Capture>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptimizeMessage {
    optimize: crate::optimize_dto::Request,
    native_hooks: Option<crate::config::hooks::Capture>,
    native_skills: Option<crate::config::review::Capture>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProjectMessage {
    project: crate::project_dto::Request,
    native_hooks: Option<crate::config::hooks::Capture>,
    native_skills: Option<crate::config::review::Capture>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Message {
    Project(ProjectMessage),
    Usage(Request),
    Config(ConfigMessage),
    Optimize(OptimizeMessage),
    Conversation {
        conversation: crate::conversation_dto::Request,
    },
    Timing {
        timing: crate::timing_dto::Request,
    },
    Artifacts {
        artifacts: crate::artifacts_dto::Request,
    },
    Monitor {
        monitor: crate::monitor::Request,
    },
}
pub fn dispatch(value: Value) -> Result<Value> {
    // Each non-usage envelope has exactly one product operation plus explicitly captured facts.
    for (key, value) in value.as_object().into_iter().flatten() {
        let product = if key == "query"
            || key == "config"
            || key == "conversation"
            || key == "timing"
            || key == "optimize"
            || key == "monitor"
        {
            value
        } else {
            continue;
        };
        if product.get("action").and_then(Value::as_str) == Some("changes")
            || product.get("snapshotId").is_some()
            || product.get("readView").is_some()
            || product.get("baselineSnapshotId").is_some()
            || product
                .get("activity")
                .and_then(|v| v.get("snapshotId"))
                .is_some()
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "View selectors are not supported; send the task scope or original baseline instead",
            ));
        }
    }
    let message: Message = serde_json::from_value(value.clone())
        .map_err(|e| operation_error("INVALID_ARGUMENT", format!("Invalid local request: {e}")))?;
    let allowed = match &message {
        Message::Config(_) | Message::Project(_) | Message::Optimize(_) => 3,
        Message::Usage(_) => usize::MAX,
        _ => 1,
    };
    if value.as_object().is_none_or(|v| v.len() > allowed) {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Unexpected local request fields",
        ));
    }
    let needs_index = match &message {
        Message::Usage(request) => !direct_report(request),
        Message::Project(_) => true,
        Message::Conversation { conversation } => {
            conversation.action != crate::conversation_dto::Action::SourceSearch
        }
        Message::Config(m) => m.config.action != crate::config_dto::Action::Capabilities,
        Message::Optimize(m) => m.optimize.action != crate::optimize_dto::Action::Capabilities,
        Message::Timing { timing } => {
            !matches!(timing, crate::timing_dto::Request::Capabilities { .. })
        }
        Message::Monitor { monitor } => matches!(monitor, crate::monitor::Request::Check { .. }),
        Message::Artifacts { .. } => false,
    };
    // Serialize the data operation, not just its write phase: parallel callers
    // wait before loading facts instead of multiplying large request heaps.
    let _request_guard = if needs_index {
        let root = directory()?;
        let guard = acquire(&root)?;
        super::generation::cleanup_pending(&root, None)?;
        Some(guard)
    } else {
        None
    };
    match message {
        Message::Project(m) => Ok(serde_json::to_value(super::project::execute(
            m.project,
            m.native_hooks.as_ref(),
            m.native_skills.as_ref(),
        )?)?),
        Message::Usage(r) => Ok(serde_json::to_value(usage(r)?)?),
        Message::Artifacts { artifacts } => Ok(serde_json::to_value(crate::artifacts::execute(
            artifacts,
            &|| false,
        )?)?),
        Message::Config(mut m) => {
            if m.config.action == crate::config_dto::Action::Capabilities {
                return Ok(serde_json::to_value(crate::config::capabilities())?);
            }
            normalize_projects(&mut m.config.project_roots, &mut m.config.scope.project)?;
            let view = config_read(&m.config, m.native_hooks.as_ref(), m.native_skills.as_ref())?;
            Ok(serde_json::to_value(crate::config::execute(
                m.config,
                String::new(),
                &view,
            )?)?)
        }
        Message::Optimize(mut m) => {
            crate::optimize::activity::validate(&m.optimize)?;
            if m.optimize.action == crate::optimize_dto::Action::Capabilities {
                let mut out = crate::optimize::capabilities();
                out.rule_parameters = crate::optimize::parameters(m.optimize.rule_overrides)?;
                return Ok(serde_json::to_value(out)?);
            }
            if m.optimize.action == crate::optimize_dto::Action::Activity {
                return Ok(serde_json::to_value(crate::optimize::activity::response(
                    timing(crate::optimize::activity::timing_request(&m.optimize)?)?,
                )?)?);
            }
            normalize_projects(&mut m.optimize.project_roots, &mut m.optimize.project)?;
            let cfg = crate::config_dto::Request {
                roots: m.optimize.roots.clone(),
                project_roots: m.optimize.project_roots.clone(),
                ..Default::default()
            };
            let view = config_read(&cfg, m.native_hooks.as_ref(), m.native_skills.as_ref())?;
            Ok(serde_json::to_value(crate::optimize::execute(
                m.optimize,
                String::new(),
                &view,
                None,
            )?)?)
        }
        Message::Conversation { mut conversation } => {
            if conversation.action == crate::conversation_dto::Action::SourceSearch {
                return Ok(serde_json::to_value(
                    crate::adapters::codex::direct_search::query(&conversation)?,
                )?);
            }
            let selected = read(&conversation.roots, Mode::Auto, false, false, false)?;
            conversation.snapshot_id = selected.freshness.revision.clone();
            let mut result = crate::conversation::query(
                &selected.snapshot,
                None,
                &conversation,
                &AtomicBool::new(false),
            )?;
            result.freshness = Some(selected.freshness);
            Ok(serde_json::to_value(result)?)
        }
        Message::Timing { timing: r } => Ok(serde_json::to_value(timing(r)?)?),
        Message::Monitor { mut monitor } => {
            if let crate::monitor::Request::Check {
                snapshot_id, roots, ..
            } = &mut monitor
            {
                let r = read(roots, Mode::Auto, false, false, false)?;
                *snapshot_id = r.freshness.revision;
                return Ok(serde_json::to_value(crate::monitor::dispatch_snapshot(
                    monitor,
                    &r.snapshot,
                    r.freshness.checked_at.as_deref(),
                )?)?);
            }
            Ok(serde_json::to_value(crate::monitor::dispatch(monitor)?)?)
        }
    }
}
