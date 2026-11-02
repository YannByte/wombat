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
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
const MAX_ITEMS: usize = 100;
const MAX_RESPONSE: usize = 256 * 1024;
fn item_id(scope: &Scope, project_identity: &str, item: &Item) -> Result<String> {
    Ok(crate::hash(serde_json::to_vec(&(
        2,
        scope,
        project_identity,
        &item.path,
        &item.owner,
        &item.kind,
        &item.assessment,
        &item.regeneration,
        &item.regeneration_version,
        &item.use_state,
        &item.observation,
        &item.reasons,
        &item.codex_evidence,
    ))?))
}
fn baseline_id(response: &Response) -> Result<String> {
    Ok(crate::hash(serde_json::to_vec(&(
        &response.scope,
        &response.project_identity,
        response.policy_version,
        &response.items,
    ))?))
}
pub(crate) fn execute(request: Request, cancelled: &dyn Fn() -> bool) -> Result<Response> {
    let host = projects::Native;
    execute_with(request, cancelled, &host)
}
fn execute_with(
    request: Request,
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
            let checked = chrono::Utc::now();
            let mut response = Response {
                output_version: 3,
                policy_version: 2,
                action: "inspect".into(),
                project_identity: project_identity.clone(),
                baseline_id: String::new(),
                checked_at: checked.to_rfc3339(),
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
            for item in &mut response.items {
                item.id = item_id(&response.scope, &response.project_identity, item)?;
            }
            response.baseline_id = baseline_id(&response)?;
            response
        }
        Request::Recheck {
            baseline,
            selection,
        }
        | Request::Apply {
            baseline,
            selection,
        } => {
            if selection.len() > MAX_ITEMS {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Too many selected artifacts",
                ));
            }
            if baseline.output_version != 3
                || baseline.policy_version != 2
                || baseline.action != "inspect"
                || baseline_id(&baseline)? != baseline.baseline_id
                || baseline.items.len() > MAX_ITEMS
                || serde_json::to_vec(&baseline)?.len() > MAX_RESPONSE
            {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Invalid original artifact plan; inspect again",
                ));
            }
            for item in &baseline.items {
                if item.id != item_id(&baseline.scope, &baseline.project_identity, item)? {
                    return Err(operation_error(
                        "INVALID_ARGUMENT",
                        "Original artifact identity does not match its scope and evidence",
                    ));
                }
            }
            let view = baseline;
            let selected: BTreeSet<_> = selection.iter().collect();
            if selected.len() != selection.len()
                || selected
                    .iter()
                    .any(|id| !view.items.iter().any(|item| &item.id == *id))
            {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "Selection must contain original artifact identities",
                ));
            }
            let owner_ok = projects::scope_root(&view.scope.project_root, view.scope.codex)
                .ok()
                .and_then(|p| crate::file_metadata::FileMetadata::read(p).ok())
                .is_some_and(|m| m.physical == view.project_identity);
            if applying
                && (selection.is_empty()
                    || selected.iter().any(|id| {
                        view.items.iter().find(|i| &i.id == *id).is_none_or(|i| {
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
            if applying {
                let targets = projects::discover(&view.scope, &mut budget, host)?;
                if view
                    .items
                    .iter()
                    .filter(|i| selected.contains(&i.id))
                    .any(|i| {
                        !std::fs::symlink_metadata(&i.path)
                            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                            && !targets.iter().any(|t| {
                                projects::text(&t.path).ok().as_deref() == Some(i.path.as_str())
                                    && t.kind == i.kind
                                    && t.owner.as_ref().and_then(|p| projects::text(p).ok())
                                        == i.owner
                            })
                    })
                {
                    return Err(operation_error(
                        "INVALID_ARGUMENT",
                        "Selected objects are outside current cleanup policy; inspect again",
                    ));
                }
            }
            let _install_guard = if applying
                && view
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
            for old in &view.items {
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
                    &view.scope,
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
                    && item.codex_evidence == old.codex_evidence
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
                ..*view.clone()
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
