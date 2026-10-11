//! Review decisions, immutable history and independent rule queries.
use super::{
    capabilities,
    detection::{detect_for, parameters},
    evaluation, registry,
    repository::connect,
    reviews, store,
};
use crate::{config::View, dto::operation_error, optimize_dto::*};
use anyhow::Result;
use std::path::Path;
pub(crate) fn execute(request: Request, id: String, view: &View) -> Result<Response> {
    let path = crate::storage::data_home()?.join("user-v1/reviews.sqlite3");
    execute_at(request, id, view, &path)
}
pub(super) fn execute_at(r: Request, id: String, view: &View, path: &Path) -> Result<Response> {
    execute_at_inner(r, id, view, path, false)
}
pub(crate) fn pending_for_handoff(r: Request, id: String, view: &View) -> Result<Response> {
    let path = crate::storage::data_home()?.join("user-v1/reviews.sqlite3");
    execute_at_inner(r, id, view, &path, true)
}
fn execute_at_inner(
    r: Request,
    id: String,
    view: &View,
    path: &Path,
    all: bool,
) -> Result<Response> {
    super::activity::validate(&r)?;
    if r.action == Action::Activity {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Activity requires the fixed-snapshot entry",
        ));
    }
    if r.project
        .as_ref()
        .is_some_and(|p| !view.projects.contains(p))
    {
        return Err(operation_error(
            "PROJECT_NOT_AUTHORIZED",
            "项目目录不在当前读取范围",
        ));
    }
    let rules = parameters(r.rule_overrides.clone())?;
    let offset = r.offset.unwrap_or(0);
    let limit = if all { 20_000 } else { r.limit.unwrap_or(50) };
    if limit == 0 || (!all && limit > 200) || offset > 9_007_199_254_740_991 {
        return Err(operation_error("INVALID_ARGUMENT", "分页无效"));
    }
    let mut db = connect(path)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut revision = store::revision(&tx)?;
    if r.decision_revision.as_ref().is_some_and(|v| v != &revision) {
        return Err(operation_error(
            "VIEW_EXPIRED",
            "处理记录版本已变化，请刷新",
        ));
    }
    let mut current = detect_for(
        view,
        &rules,
        r.project.as_deref(),
        r.source_instance_id.as_deref(),
    );
    let accessible = |i: &crate::config_dto::Item| {
        i.applies(r.source_instance_id.as_deref(), r.project.as_deref())
    };
    store::authorize(
        &tx,
        view.items
            .iter()
            .filter(|i| {
                accessible(i)
                    || (i.kind == crate::config_dto::Kind::Hook
                        && r.source_instance_id
                            .as_deref()
                            .is_none_or(|source| i.in_source(source)))
            })
            .map(|i| (i.id.as_str(), i.id.as_str())),
    )?;
    let mut states = store::states(&tx, r.project.as_deref())?;
    current.retain(|s| accessible(&s.item));
    for s in &mut current {
        s.scope_project = r.project.clone();
        s.id = crate::hash(format!("{}:{}", s.id, r.project.as_deref().unwrap_or("")));
    }
    // Preserve observed problem versions before later manual changes. These
    // facts are not handling history until a decision or recheck is performed.
    for suggestion in &mut current {
        if states
            .get(&suggestion.id)
            .is_none_or(|old| old.status == "verified" && !old.decided)
        {
            store::append(&tx, suggestion, RecordKind::Observation)?;
        }
    }
    states = store::states(&tx, r.project.as_deref())?;
    if matches!(
        r.action,
        Action::Keep | Action::NotApplicable | Action::Redisplay
    ) {
        reviews::decide(&tx, &r, &current, &states)?;
    }
    if r.action == Action::Recheck {
        reviews::recheck(&tx, &r, &current, &states, view, &rules)?;
    }
    revision = store::revision(&tx)?;
    let latest = store::states(&tx, r.project.as_deref())?;
    let mut pending_current = Vec::with_capacity(current.len());
    for suggestion in current {
        let suppressed = match latest.get(&suggestion.id) {
            Some(state) if state.decided => {
                let old = store::get(&tx, state.seq)?;
                suppresses(&old, &suggestion)
            }
            Some(_) => false,
            None => false,
        };
        if !suppressed {
            pending_current.push(suggestion);
        }
    }
    let mut current = pending_current;
    for state in latest
        .values()
        .filter(|s| !s.decided && s.status == "recheckUnavailable")
    {
        let previous = store::get(&tx, state.seq)?;
        if !current.iter().any(|s| s.item.id == previous.item.id) {
            current.push(previous);
        }
    }
    let pending = current.len();
    if all && pending > limit {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Pending handoff exceeds the selection limit; choose a project scope",
        ));
    }
    let history = store::history_count(&tx, r.project.as_deref())?;
    if r.action == Action::Checks {
        let items: Vec<_> = view
            .items
            .iter()
            .filter(|i| accessible(i) && r.item_id.as_ref().is_none_or(|id| &i.id == id))
            .collect();
        if r.item_id.is_some() && items.is_empty() {
            return Err(operation_error("NOT_FOUND", "未找到检查对象"));
        }
        let checks = items
            .iter()
            .skip(offset)
            .take(limit)
            .flat_map(|i| {
                evaluation::evaluate(
                    &evaluation::Input::initial(view, i, &rules, r.project.as_deref())
                        .with_source(r.source_instance_id.as_deref()),
                )
            })
            .collect();
        let total = items.len();
        tx.commit()?;
        let mut response = capabilities();
        response.capabilities = observed_capabilities(view);
        response.action = Action::Checks;
        response.read_view = Some(id);
        response.config_revision = view.revision.clone();
        response.decision_revision = revision;
        response.usage_revision = view
            .snapshot
            .as_ref()
            .map(|s| s.manifest.snapshot_ref.snapshot_id.clone());
        response.checked_at = view.checked.clone();
        response.pending = pending;
        response.history = history;
        response.page = crate::usage_app_dto::Page {
            offset,
            limit,
            total,
            next_offset: (offset.saturating_add(limit) < total)
                .then_some(offset.saturating_add(limit)),
        };
        response.result_status = "partial".into();
        response.issues = view.issues.clone();
        response.checks = checks;
        response.rule_parameters = rules;
        return Ok(response);
    }
    let (total, mut suggestions) = if r.group == Group::History {
        store::page(&tx, &r, offset, limit)?
    } else {
        if r.action == Action::Detail {
            current.retain(|s| Some(&s.id) == r.suggestion_id.as_ref());
        }
        if let Some(category) = &r.category {
            current.retain(|s| &s.category == category);
        }
        let total = current.len();
        (
            total,
            current
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect::<Vec<_>>(),
        )
    };
    if r.action == Action::Detail && total == 0 {
        return Err(operation_error("NOT_FOUND", "未找到建议"));
    }
    if r.group != Group::History {
        for s in &mut suggestions {
            if let Some(old) = latest.get(&s.id) {
                *s = store::get(&tx, old.seq)?;
            }
        }
    }
    let end = offset.saturating_add(limit).min(total);
    tx.commit()?;
    let capabilities = observed_capabilities(view);
    let follow_ups =
        super::follow_up::observe(&suggestions, view, r.source_instance_id.as_deref())?;
    let mut issues = view.issues.clone();
    issues.extend(
        [
            "inactivityCoverageUnavailable",
            "mcpFaultEvidenceUnavailable",
            "spaceAdapterUnavailable",
        ]
        .map(|code| crate::config_dto::Issue {
            code: code.into(),
            path: None,
        }),
    );
    Ok(Response {
        output_version: crate::optimize_dto::OUTPUT_VERSION,
        action: r.action,
        capabilities,
        read_view: Some(id),
        config_revision: view.revision.clone(),
        usage_revision: view
            .snapshot
            .as_ref()
            .map(|s| s.manifest.snapshot_ref.snapshot_id.clone()),
        decision_revision: revision,
        checked_at: view.checked.clone(),
        suggestions,
        pending,
        history,
        page: crate::usage_app_dto::Page {
            offset,
            limit,
            total,
            next_offset: (end < total).then_some(end),
        },
        issues,
        result_status: "partial".into(),
        rule_parameters: rules,
        rule_catalog: registry::catalog(),
        checks: vec![],
        follow_ups,
        activity: None,
    })
}

fn observed_capabilities(view: &View) -> Capabilities {
    let hook_support = match view.hook_registry.status {
        crate::config_dto::HookRegistryStatus::Unavailable => HookSupport::default(),
        ref status => HookSupport {
            host: Some("codex".into()),
            host_version: view.hook_registry.native_version.clone(),
            effective_registry: true,
            status: if matches!(status, crate::config_dto::HookRegistryStatus::Observed) {
                HookSupportStatus::RegistryObserved
            } else {
                HookSupportStatus::RegistryPartial
            },
        },
    };
    Capabilities {
        hook_support,
        ..Capabilities::default()
    }
}

/// Current hits are hidden only by a still-applicable explicit user decision.
/// A historical verified miss cannot suppress a recurrence.
pub(super) fn suppresses(previous: &Suggestion, current: &Suggestion) -> bool {
    previous
        .decision
        .as_ref()
        .is_some_and(|decision| super::identity::decision_applies(decision, current))
}
