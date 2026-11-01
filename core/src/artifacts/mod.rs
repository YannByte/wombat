//! Current filesystem evidence, short-lived baselines and selected Codex cleanup.
mod codex;
#[cfg(all(test, unix))]
mod codex_tests;
mod projects;
mod scan;
#[cfg(test)]
mod tests;
use crate::{artifacts_dto::*, dto::operation_error};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const TTL: u64 = 600;
const MAX_ITEMS: usize = 100;
const MAX_RESPONSE: usize = 256 * 1024;
const MAX_VIEWS: usize = 8;
const MAX_VIEW_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone)]
struct View {
    response: Response,
    project_identity: String,
}
#[derive(Default)]
pub(crate) struct Store {
    views: VecDeque<(String, Instant, Arc<View>, usize)>,
}
impl Store {
    pub(crate) fn has_views(&mut self) -> bool {
        self.views
            .retain(|(_, at, _, _)| at.elapsed() < Duration::from_secs(TTL));
        !self.views.is_empty()
    }
    fn get(&mut self, id: &str) -> Result<Arc<View>> {
        self.has_views();
        self.views
            .iter()
            .find(|(key, _, _, _)| key == id)
            .map(|(_, _, v, _)| Arc::clone(v))
            .ok_or_else(|| {
                operation_error(
                    "VIEW_EXPIRED",
                    "Artifact view expired; inspect the original scope again",
                )
            })
    }
    fn insert(&mut self, view: View) -> Result<()> {
        let bytes = serde_json::to_vec(&view.response)?.len();
        if bytes > MAX_RESPONSE {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Artifact view exceeds its response budget",
            ));
        }
        while self.views.len() >= MAX_VIEWS
            || self.views.iter().map(|v| v.3).sum::<usize>() + bytes > MAX_VIEW_BYTES
        {
            self.views.pop_front();
        }
        self.views.push_back((
            view.response.read_view.clone(),
            Instant::now(),
            Arc::new(view),
            bytes,
        ));
        Ok(())
    }
}
pub(crate) fn execute(
    request: Request,
    store: &Mutex<Store>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response> {
    let host = projects::Native;
    execute_with(request, store, cancelled, &host)
}
fn execute_with(
    request: Request,
    store: &Mutex<Store>,
    cancelled: &dyn Fn() -> bool,
    host: &dyn projects::Host,
) -> Result<Response> {
    let applying = matches!(&request, Request::Apply { .. });
    let mut budget = scan::Budget::new(cancelled);
    let response = match request {
        Request::Inspect {
            project_root,
            paths,
            keep_paths,
            product_temporary,
            codex,
        } => {
            if paths.len() > 50 || keep_paths.len() > 50 || codex && product_temporary {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Select at most 50 paths",
                ));
            }
            let root = projects::scope_root(&project_root, codex)?;
            let project_identity = crate::file_metadata::FileMetadata::read(&root)?.physical;
            let scope = Scope {
                project_root: projects::text(&root)?,
                paths: normalize(&paths, &root)?,
                keep_paths: normalize(&keep_paths, &root)?,
                product_temporary,
                codex,
            };
            let targets = projects::discover(&scope, &mut budget, host)?;
            let mut files = BTreeMap::new();
            let mut items = vec![];
            for target in targets {
                budget.check()?;
                items.push(scan::inspect(
                    &scope,
                    target,
                    &mut budget,
                    host,
                    &mut files,
                )?);
            }
            let id = format!("artifacts:{}", uuid::Uuid::new_v4());
            let checked = chrono::Utc::now();
            let mut response = Response {
                output_version: 2,
                policy_version: 2,
                action: "inspect".into(),
                read_view: id.clone(),
                baseline_id: crate::hash(serde_json::to_vec(&(&id, &scope, 2))?),
                checked_at: checked.to_rfc3339(),
                expires_at: (checked + chrono::Duration::seconds(TTL as i64)).to_rfc3339(),
                scope,
                items,
                summary: empty_summary(),
                partial: budget.partial,
            };
            if crate::file_metadata::FileMetadata::read(&root)
                .ok()
                .is_none_or(|m| m.physical != project_identity)
            {
                for item in &mut response.items {
                    item.assessment = Assessment::Unknown;
                    item.reasons.push(Reason::OwnerChanged);
                }
                budget.partial = true;
            }
            finish(&mut response, &files, budget.partial)?;
            budget.check()?;
            let view = View {
                project_identity,
                response: response.clone(),
            };
            store
                .lock()
                .map_err(|_| operation_error("CORE_ERROR", "Artifact view lock unavailable"))?
                .insert(view)?;
            response
        }
        Request::Recheck {
            read_view,
            baseline_id,
            selection,
        }
        | Request::Apply {
            read_view,
            baseline_id,
            selection,
        } => {
            if selection.len() > MAX_ITEMS {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Too many selected artifacts",
                ));
            }
            let view = store
                .lock()
                .map_err(|_| operation_error("CORE_ERROR", "Artifact view lock unavailable"))?
                .get(&read_view)?;
            if baseline_id != view.response.baseline_id {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Artifact baseline does not match its original view",
                ));
            }
            let selected: BTreeSet<_> = selection.iter().collect();
            if selected.len() != selection.len()
                || selected
                    .iter()
                    .any(|id| !view.response.items.iter().any(|item| &item.id == *id))
            {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Selection must contain original artifact identities",
                ));
            }
            let owner_ok =
                projects::scope_root(&view.response.scope.project_root, view.response.scope.codex)
                    .ok()
                    .and_then(|p| crate::file_metadata::FileMetadata::read(p).ok())
                    .is_some_and(|m| m.physical == view.project_identity);
            if applying
                && (selection.is_empty()
                    || selected.iter().any(|id| {
                        view.response
                            .items
                            .iter()
                            .find(|i| &i.id == *id)
                            .is_none_or(|i| {
                                !codex::is_kind(&i.kind)
                                    || i.assessment != Assessment::Candidate
                                    || i.observation.as_ref().is_none_or(|o| !o.complete)
                            })
                    }))
            {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Apply requires explicit, complete Codex candidates from the original view",
                ));
            }
            let _install_guard = if applying
                && view
                    .response
                    .items
                    .iter()
                    .any(|i| selected.contains(&i.id) && i.kind == Kind::CodexDaemonRelease)
            {
                Some(codex::lock(&host.codex_paths()).map_err(|_| {
                    operation_error(
                        "UPDATE_BUSY",
                        "Codex installation lock is busy or unavailable; inspect again",
                    )
                })?)
            } else {
                None
            };
            budget.daemon_locked = _install_guard.is_some();
            budget.daemon_lock_identity = _install_guard
                .as_ref()
                .map(crate::file_metadata::FileMetadata::from_file)
                .transpose()?
                .map(|m| m.physical);
            let mut stopped = false;
            let mut files = BTreeMap::new();
            let mut items = vec![];
            for old in &view.response.items {
                if !selected.is_empty() && !selected.contains(&old.id) {
                    continue;
                }
                budget.check()?;
                if applying && stopped {
                    let mut item = old.clone();
                    item.observation = None;
                    item.assessment = Assessment::Unknown;
                    item.recheck = Some(RecheckState::Unavailable);
                    item.removal = Some(Removal::Skipped);
                    item.reasons = vec![Reason::ConditionsChanged];
                    items.push(item);
                    continue;
                }
                if !owner_ok {
                    let mut item = old.clone();
                    item.observation = None;
                    item.assessment = Assessment::Unknown;
                    item.use_state = UseState::Unknown;
                    item.reasons = vec![Reason::OwnerChanged];
                    item.recheck = Some(RecheckState::Unavailable);
                    if applying {
                        item.removal = Some(Removal::Skipped);
                        stopped = true;
                    }
                    items.push(item);
                    continue;
                }
                if applying {
                    budget.codex_process.clear();
                    budget.codex_build = None;
                }
                let path = PathBuf::from(&old.path);
                let mut item_files = BTreeMap::new();
                let mut item = scan::inspect(
                    &view.response.scope,
                    projects::Target {
                        path,
                        kind: old.kind.clone(),
                        owner: old.owner.as_ref().map(PathBuf::from),
                    },
                    &mut budget,
                    host,
                    if applying {
                        &mut item_files
                    } else {
                        &mut files
                    },
                )?;
                item.id = old.id.clone();
                item.recheck = Some(if old.observation.as_ref().is_none_or(|o| !o.complete) {
                    RecheckState::Unavailable
                } else if item.reasons.contains(&Reason::NotFound) {
                    RecheckState::Gone
                } else if item.observation.as_ref().is_none_or(|o| !o.complete)
                    || item.assessment == Assessment::Unknown
                {
                    RecheckState::Unavailable
                } else if item.observation == old.observation
                    && item.assessment == old.assessment
                    && item.use_state == old.use_state
                    && item.regeneration == old.regeneration
                    && item.regeneration_version == old.regeneration_version
                    && item.reasons == old.reasons
                {
                    RecheckState::Unchanged
                } else {
                    RecheckState::Changed
                });
                if applying {
                    if stopped {
                        item.removal = Some(Removal::Skipped);
                        item.reasons.push(Reason::ConditionsChanged);
                    } else if item.recheck == Some(RecheckState::Gone) {
                        item.removal = Some(Removal::AlreadyAbsent);
                    } else if item.recheck == Some(RecheckState::Unchanged)
                        && item.assessment == Assessment::Candidate
                    {
                        match codex::remove(&item, cancelled) {
                            Ok(()) => {
                                item.removal = Some(Removal::Removed);
                                item.observation = None;
                                item.assessment = Assessment::Absent;
                                item.recheck = Some(RecheckState::Gone);
                                item.reasons = vec![Reason::NotFound];
                            }
                            Err(error) => {
                                if cancelled() {
                                    return Err(error);
                                }
                                item.observation = None;
                                item.assessment = Assessment::Unknown;
                                item.recheck = Some(RecheckState::Unavailable);
                                item.removal = Some(Removal::Failed);
                                item.reasons.push(Reason::RemovalFailed);
                                stopped = true;
                            }
                        }
                    } else {
                        item.removal = Some(Removal::Skipped);
                        item.reasons.push(Reason::ConditionsChanged);
                        stopped = true;
                    }
                }
                if applying && item.observation.is_some() {
                    files.extend(item_files);
                }
                items.push(item);
            }
            let mut response = Response {
                action: if applying { "apply" } else { "recheck" }.into(),
                checked_at: chrono::Utc::now().to_rfc3339(),
                items,
                summary: empty_summary(),
                partial: false,
                ..view.response.clone()
            };
            finish(
                &mut response,
                &files,
                budget.partial || !owner_ok || stopped,
            )?;
            response
        }
    };
    budget.check()?;
    if serde_json::to_vec(&response)?.len() > MAX_RESPONSE {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Artifact result exceeds its response budget; narrow the scope",
        ));
    }
    Ok(response)
}
fn normalize(paths: &[String], root: &Path) -> Result<Vec<String>> {
    let mut result = BTreeSet::new();
    for p in paths {
        if p.is_empty() || p.contains('\0') {
            return Err(operation_error("INVALID_ARGUMENT", "Invalid artifact path"));
        }
        let p = Path::new(p);
        result.insert(projects::text(&crate::absolute(if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        })?)?);
    }
    Ok(result.into_iter().collect())
}
fn empty_summary() -> Summary {
    Summary {
        candidates: 0,
        review: 0,
        protected: 0,
        unknown: 0,
        absent: 0,
        observed_logical_bytes: 0,
        complete: false,
    }
}
fn finish(response: &mut Response, files: &BTreeMap<String, u64>, partial: bool) -> Result<()> {
    response.items.sort_by(|a, b| a.path.cmp(&b.path));
    response.summary = empty_summary();
    for i in &response.items {
        match i.assessment {
            Assessment::Candidate => response.summary.candidates += 1,
            Assessment::Review => response.summary.review += 1,
            Assessment::Protected => response.summary.protected += 1,
            Assessment::Unknown => response.summary.unknown += 1,
            Assessment::Absent => response.summary.absent += 1,
        };
    }
    response.summary.observed_logical_bytes = files.values().try_fold(0_u64, |sum, n| {
        sum.checked_add(*n)
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Artifact byte count overflow"))
    })?;
    response.partial = partial
        || response.items.iter().any(|i| {
            i.assessment == Assessment::Unknown
                || i.reasons
                    .iter()
                    .any(|r| matches!(r, Reason::UseUnknown | Reason::GitUnavailable))
                || i.observation.as_ref().is_some_and(|o| !o.complete)
        });
    response.summary.complete = !response.partial;
    Ok(())
}
