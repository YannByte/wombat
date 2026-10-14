//! Route typed usage and configuration queries against selected immutable revisions.
use super::*;
pub(super) fn monitor_query(
    request: crate::monitor::Request,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
) -> Result<crate::monitor::Response> {
    let crate::monitor::Request::Check { snapshot_id, .. } = &request else {
        return crate::monitor::dispatch(request);
    };
    let selector = selection::ReadViewSelector::new(
        vec![],
        Some(snapshot_id.clone()),
        Mode::Cached,
        false,
        false,
    )?;
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let (snapshot, freshness) = select_with_retained(&selector, shared, jobs, configs, &cancelled)?;
    if freshness.error_code.is_some() || freshness.initial_scan || freshness.status == "syncing" {
        return Err(operation_error(
            "SYNC_PENDING",
            "Monitor requires a ready source check",
        ));
    }
    crate::monitor::dispatch_snapshot(request, &snapshot, freshness.checked_at.as_deref())
}
pub(super) fn query(
    request: Request,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
) -> Result<Response> {
    let mut validation = request.query.clone();
    validation.roots = None;
    crate::usage_app::validate(&validation)?;
    let refresh = request.query.action == usage_app_dto::Action::Refresh;
    if request.verify && !refresh {
        return Err(operation_error("INVALID_ARGUMENT", "实时验证仅适用于更新"));
    }
    let selector = selection::ReadViewSelector::new(
        request.query.roots.clone().unwrap_or_default(),
        request.query.snapshot_id.clone(),
        if matches!(
            request.query.action,
            usage_app_dto::Action::Compare
                | usage_app_dto::Action::Investigate
                | usage_app_dto::Action::Trajectory
                | usage_app_dto::Action::Resources
                | usage_app_dto::Action::Review
                | usage_app_dto::Action::Context
        ) && request.mode == Mode::Auto
        {
            Mode::Cached
        } else {
            request.mode.clone()
        },
        request.verify,
        refresh,
    )?
    .project(request.query.scope.project.clone());
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let (snapshot, freshness) = select_with_retained(&selector, shared, jobs, configs, &cancelled)?;
    let mut query = request.query;
    query.roots = None;
    query.snapshot_id = None;
    let mut result = if query.action == usage_app_dto::Action::Refresh {
        // Export only on an explicit refresh. Automatic updates never create snapshots.
        let _lock = crate::usage_store::RefreshLock::acquire()?;
        let saved = snapshot.export_live()?;
        // The exporter has checked the price catalog and saved these same facts.
        // Do not reread the ledger or construct period rows only to discard them.
        let rows = snapshot.ledger()?;
        crate::usage_app::refresh_response(&saved, &rows.iter().collect::<Vec<_>>())?
    } else {
        crate::usage_app::execute_snapshot(query, &snapshot)?
    };
    result.freshness = Some(freshness.clone());
    let response = Response {
        output_version: 1,
        result,
        freshness,
    };
    if response.result.inspection.is_some()
        && serde_json::to_vec(&response)?.len() > usage_app_dto::INSPECTION_RESPONSE_BYTES
    {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "检查查询含实时元数据后超过响应预算",
        ));
    }
    Ok(response)
}
fn select_with_retained(
    selector: &selection::ReadViewSelector,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(Arc<Snapshot>, Freshness)> {
    select_view(selector, shared, jobs, cancelled).or_else(|error| {
        if error
            .downcast_ref::<crate::dto::OperationError>()
            .is_some_and(|e| e.code == "VIEW_EXPIRED")
            && let Some(id) = selector.identity()
            && let Some((snapshot, checked_at)) = configs.lock().unwrap().snapshot(id)
        {
            let initial_scan = preview::is_initial(&snapshot);
            return Ok((
                Arc::clone(&snapshot),
                Freshness {
                    publication_change: snapshot.publication_change.clone(),
                    project_loads: snapshot.project_loads.clone(),
                    initial_scan,
                    status: "fixed".into(),
                    checked_at: Some(checked_at),
                    revision: id.clone(),
                    error: None,
                    error_code: None,
                },
            ));
        }
        Err(error)
    })
}

pub(super) fn conversation_query(
    request: crate::conversation_dto::Request,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<crate::conversation_dto::Response> {
    crate::conversation::validate(&request)?;
    let selector = selection::ReadViewSelector::new(
        request.roots.clone(),
        Some(request.snapshot_id.clone()),
        Mode::Cached,
        false,
        false,
    )?;
    let (snapshot, freshness) = select_with_retained(&selector, shared, jobs, configs, cancelled)?;
    let baseline = request
        .baseline_snapshot_id
        .as_ref()
        .map(|id| {
            let selector = selection::ReadViewSelector::new(
                request.roots.clone(),
                Some(id.clone()),
                Mode::Cached,
                false,
                false,
            )?;
            select_with_retained(&selector, shared, jobs, configs, cancelled).map(|(s, _)| s)
        })
        .transpose()?;
    let mut result =
        crate::conversation::query(&snapshot, baseline.as_deref(), &request, cancelled)?;
    result.freshness = Some(freshness);
    if serde_json::to_vec(&result)?.len() > 1024 * 1024 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Conversation response exceeds budget",
        ));
    }
    Ok(result)
}

pub(super) fn config_query(
    request: crate::config_dto::Request,
    native: Option<&crate::config::hooks::Capture>,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
) -> Result<crate::config_dto::Response> {
    crate::config::validate(&request)?;
    if request.action == crate::config_dto::Action::Capabilities {
        return Ok(crate::config::capabilities());
    }
    let (id, view) = if let Some(id) = &request.read_view {
        let view = configs.lock().unwrap().get(id)?;
        if request.roots.as_deref().unwrap_or_default() != view.roots
            || request.project_roots.as_deref().unwrap_or_default() != view.project_roots
        {
            return Err(operation_error("INVALID_ARGUMENT", "来源范围不匹配"));
        }
        (id.clone(), view)
    } else {
        let selector = selection::ReadViewSelector::new(
            request.roots.clone().unwrap_or_default(),
            request.snapshot_id.clone(),
            Mode::Auto,
            false,
            false,
        )?
        .capture_now();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let selected = select_view(&selector, shared, jobs, &cancelled);
        let (snapshot, status) = match selected {
            Ok((snapshot, freshness)) => (Some(snapshot), freshness.status),
            Err(error) if request.snapshot_id.is_some() => return Err(error),
            Err(error)
                if error
                    .downcast_ref::<crate::dto::OperationError>()
                    .is_some_and(|error| error.code == "SYNC_PENDING") =>
            {
                (None, "syncing".into())
            }
            Err(_) => (None, "unavailable".into()),
        };
        let view = crate::config::prepare_observed(&request, snapshot, status, native)?;
        configs.lock().unwrap().insert(view)
    };
    crate::config::execute(request, id, &view)
}

pub(super) fn timing_query(
    request: crate::timing_dto::Request,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    configs: &Mutex<crate::config::Store>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<crate::timing_dto::Response> {
    use crate::timing_dto::{Mode as TimingMode, Request as TimingRequest};
    crate::timing::validate(&request)?;
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(operation_error("CANCELLED", "Cancelled"));
    }
    let (roots, identity, mode) = match &request {
        TimingRequest::Capabilities { privacy_profile } => {
            return crate::timing::capabilities(*privacy_profile);
        }
        TimingRequest::Summary {
            roots,
            snapshot_id,
            mode,
            ..
        } => (
            roots.clone(),
            snapshot_id.clone(),
            match mode {
                TimingMode::Auto => Mode::Auto,
                TimingMode::Fresh => Mode::Fresh,
                TimingMode::Cached => Mode::Cached,
            },
        ),
        TimingRequest::Evidence {
            roots, snapshot_id, ..
        } => (roots.clone(), Some(snapshot_id.clone()), Mode::Cached),
    };
    let selector = selection::ReadViewSelector::new(roots, identity, mode, false, false)?;
    let (snapshot, freshness) = select_with_retained(&selector, shared, jobs, configs, cancelled)?;
    crate::timing::query_on_snapshot(
        &snapshot,
        &request,
        crate::timing_dto::QueryFreshness {
            status: freshness.status,
            checked_at: freshness.checked_at,
            // The live revision is an opaque snapshot identity, not a numeric sequence.
            revision: None,
            error_code: freshness.error_code,
        },
        cancelled,
    )
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod timing_tests;
