//! Inventory lists, usage summaries, evidence and related scopes from one pinned view.
use super::*;
use crate::usage_observations::{self, Projection, TimeBasis};
mod history;
mod response;

pub(crate) fn execute(mut r: Request, id: String, view: &View) -> Result<Response> {
    validate(&r)?;
    if r.review && matches!(r.sort, Sort::Tokens) {
        r.sort = Sort::Review;
    }
    let (scope, tz) = normalize_at(&r.scope, &view.checked)?;
    let mut result = query_response(&r, id, &scope, view);
    if scope
        .project
        .as_ref()
        .is_some_and(|p| !view.projects.contains(p))
    {
        result.coverage.issues.push(Issue {
            code: "projectNotAuthorized".into(),
            path: scope.project.clone(),
        });
        return Ok(result);
    }
    let mut items = project_inventory(view, &scope);
    let history = history::project_history(&r, &scope, tz, view, &mut items, &mut result)?;
    response::publish_query(&r, &scope, view, items, result, history)
}

fn query_response(r: &Request, id: String, scope: &Scope, view: &View) -> Response {
    let mut result = capabilities();
    result.hook_registry = view.hook_registry.clone();
    result
        .hook_registry
        .contexts
        .retain(|c| scope.project.as_ref().is_none_or(|p| p == &c.project));
    result.action = r.action.clone();
    result.scope = scope.clone();
    result.checked_at = view.checked.clone();
    result.read_view = Some(id);
    result.config_revision = view.revision.clone();
    result.authorized_projects = view.projects.clone();
    result.authorized_source_roots = view.roots.clone();
    result.usage_revision = view
        .snapshot
        .as_ref()
        .map(|s| s.manifest.snapshot_ref.snapshot_id.clone());
    result.coverage.history_status = if view.snapshot.as_ref().is_some_and(|s| {
        s.manifest
            .issues
            .iter()
            .any(|issue| issue.code == crate::adapters::codex::preview::ISSUE)
    }) {
        "syncing".into()
    } else if view.snapshot.as_ref().is_some_and(|s| {
        !s.manifest.issues.is_empty()
            || s.manifest
                .sources
                .iter()
                .any(|s| s.status != "complete" || !s.issues.is_empty())
    }) {
        "partial".into()
    } else {
        view.history_status.clone()
    };
    result.coverage.issues = view.issues.clone();
    // Config precedence, explicit adoption and missing event types cannot prove absence.
    result.coverage.status = "partial".into();
    result.coverage.issues.push(Issue {
        code: "historyCoverageUnknown".into(),
        path: None,
    });
    result
}

fn project_inventory(view: &View, scope: &Scope) -> Vec<Item> {
    let mut items = view
        .items
        .iter()
        .filter(|i| applicable(i, scope))
        .cloned()
        .collect::<Vec<_>>();
    // Cached inventory metadata cannot carry a prior query's historical use state.
    for item in &mut items {
        if let Some(footprint) = &mut item.context_footprint {
            footprint
                .contexts
                .retain(|c| scope.project.as_ref().is_none_or(|p| p == &c.project));
        }
        item.usage_count = None;
        item.use_basis = Some(usage_observations::basis(
            None,
            if item.kind == Kind::Rule {
                UseUnit::RuleLoadOrRead
            } else {
                UseUnit::ObjectUse
            },
            use_scope(item, scope),
            &view.checked,
            view.snapshot.as_deref(),
            scope.all_time != Some(true),
        ));
        item.counts = Counts::default();
        item.observation = Observation::Unknown;
        item.last_record_at = None;
        item.related_turns = 0;
        item.related_tasks = 0;
        item.usage = None;
        for context in &mut item.source_contexts {
            context.counts = Counts::default();
            context.observation = Observation::Unknown;
            context.last_record_at = None;
        }
    }
    items
}

fn catalog_occurrence(operation: &crate::adapters::contract::Operation) -> String {
    format!(
        "{}:{}:{}:{}",
        operation.thread_id,
        operation.turn_id.as_deref().unwrap_or(""),
        operation.sequence,
        operation.response_id.as_deref().unwrap_or("")
    )
}

fn normalized_operation_path(path: Option<&str>, project: Option<&str>) -> Option<String> {
    usage_observations::normalized_path(path?, project)
}

fn use_scope(item: &Item, scope: &Scope) -> UseScope {
    UseScope {
        source_instance_ids: item
            .source_ids()
            .filter(|id| {
                scope
                    .source_instance_id
                    .as_deref()
                    .is_none_or(|source| source == *id)
            })
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        project: scope.project.clone(),
        thread_id: scope.thread_id.clone(),
        agent_kind: scope.agent_kind.clone(),
        window: if scope.all_time == Some(true) {
            UseWindow::AllHistory
        } else {
            UseWindow::DateWindow {
                since: scope.since.clone().expect("normalized date scope"),
                until: scope.until.clone().expect("normalized date scope"),
                timezone: scope.timezone.clone().expect("normalized timezone"),
            }
        },
    }
}

fn extension_activity_items(
    items: &[Item],
    scope: &Scope,
    checked: &str,
) -> Result<Vec<ExtensionActivity>> {
    use chrono::TimeZone;
    let tz: Tz = scope.timezone.as_deref().unwrap_or("UTC").parse()?;
    let cutoff = DateTime::parse_from_rfc3339(checked)?.with_timezone(&Utc);
    let boundary = |value: &Option<String>| {
        value
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .and_then(|d| tz.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).earliest())
            .map(|d| d.with_timezone(&Utc))
    };
    let end = boundary(&scope.until).map_or(cutoff, |end| end.min(cutoff));
    Ok(items
        .iter()
        .filter(|item| matches!(item.kind, Kind::Skill | Kind::Mcp))
        .map(|item| {
            let last = item
                .last_record_at
                .as_deref()
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.with_timezone(&Utc));
            let start = last.or_else(|| boundary(&scope.since));
            ExtensionActivity {
                item_id: item.id.clone(),
                observed_records: item.usage_count,
                no_observed_use_days: item
                    .usage_count
                    .and(start)
                    .filter(|start| *start <= end)
                    .map(|start| (end - start).num_seconds() as u64 / 86400),
                last_record_at: item.last_record_at.clone(),
                absence_observable: false,
                use_basis: item.use_basis.clone(),
            }
        })
        .collect())
}
