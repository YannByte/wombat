//! Query filtering and pagination preserve full-scope totals before selecting a page.
use super::history::HistoricalEvidence;
use super::*;

pub(super) fn publish_query(
    r: &Request,
    scope: &Scope,
    view: &View,
    mut items: Vec<Item>,
    mut result: Response,
    history: HistoricalEvidence,
) -> Result<Response> {
    let HistoricalEvidence {
        mut evidence,
        uncertain_items,
    } = history;
    if scope.thread_id.is_some() {
        items.retain(|i| {
            i.counts.file_reads + i.counts.tool_calls + i.counts.resource_reads > 0
                || i.usage_count.unwrap_or(0) > 0
                || i.observation == Observation::LoadedOnly
                || uncertain_items.contains(&i.id)
        });
    }
    result.summary.current_items = items
        .iter()
        .filter(|i| i.current && i.configured_state != "missing")
        .count();
    result.summary.historical_items = items.iter().filter(|i| !i.current).count();
    result.summary.observed_items = items
        .iter()
        .filter(|i| i.observation == Observation::Used)
        .count();
    items.retain(|i| {
        r.kind.as_ref().is_none_or(|k| k == &i.kind)
            && r.kinds.as_ref().is_none_or(|kinds| kinds.contains(&i.kind))
            && r.observation.as_ref().is_none_or(|v| v == &i.observation)
            && r.search.as_ref().is_none_or(|q| {
                format!("{} {}", i.name, i.path)
                    .to_lowercase()
                    .contains(&q.to_lowercase())
            })
    });
    items.sort_by(|a, b| {
        let order = match r.sort {
            Sort::Tokens => b
                .usage
                .as_ref()
                .and_then(|v| v.available_token_subtotal())
                .cmp(&a.usage.as_ref().and_then(|v| v.available_token_subtotal())),
            Sort::Activity => b.usage_count.cmp(&a.usage_count),
            Sort::Size => b.bytes.cmp(&a.bytes),
            Sort::Name => a.name.cmp(&b.name),
            Sort::ContentTokens => b.content_tokens.cmp(&a.content_tokens),
            Sort::Characters => b.characters.cmp(&a.characters),
            Sort::Recent => b.last_record_at.cmp(&a.last_record_at),
            Sort::Review => review::compare(a, b),
            Sort::ContextTokens => review::tokens(b).cmp(&review::tokens(a)),
            Sort::LeastActivity => a
                .usage_count
                .is_none()
                .cmp(&b.usage_count.is_none())
                .then(a.usage_count.cmp(&b.usage_count)),
        };
        order.then(a.id.cmp(&b.id))
    });
    let all_activity = extension_activity_items(&items, scope, &view.checked)?;
    if r.review {
        result.extension_review = Some(review::summary(&items));
    }
    let activity_counts = (
        all_activity
            .iter()
            .filter(|i| i.observed_records.is_some_and(|n| n > 0))
            .count(),
        all_activity
            .iter()
            .filter(|i| i.observed_records == Some(0))
            .count(),
        all_activity
            .iter()
            .filter(|i| i.observed_records.is_none())
            .count(),
    );
    let offset = r.offset.unwrap_or(0);
    let limit = r.limit.unwrap_or(50);
    if !matches!(r.action, Action::List | Action::Explain) || r.item_id.is_some() {
        items.retain(|i| Some(&i.id) == r.item_id.as_ref());
        if items.is_empty() {
            return Err(operation_error("NOT_FOUND", "未找到配置"));
        }
    }
    evidence.sort_by(|a, b| (&a.timestamp, &a.id).cmp(&(&b.timestamp, &b.id)));
    let total = if r.action == Action::Explain {
        result
            .explanation
            .as_ref()
            .map_or(0, |explanation| explanation.page.total)
    } else if r.action == Action::Evidence {
        evidence.len()
    } else if r.action == Action::RelatedScopes {
        result.related_scopes.len()
    } else {
        items.len()
    };
    result.page = crate::usage_app_dto::Page {
        offset,
        limit,
        total,
        next_offset: (offset.saturating_add(limit) < total).then_some(offset.saturating_add(limit)),
    };
    result.items = if r.action == Action::Explain {
        items.into_iter().take(limit).collect()
    } else if r.action == Action::List {
        items.into_iter().skip(offset).take(limit).collect()
    } else {
        items
    };
    result.evidence = if r.action == Action::Evidence {
        evidence.into_iter().skip(offset).take(limit).collect()
    } else {
        vec![]
    };
    if r.action != Action::RelatedScopes {
        result.related_scopes.clear();
    } else {
        result.related_scopes = result
            .related_scopes
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
    }
    if matches!(r.action, Action::List | Action::Detail) {
        let mut activity = ExtensionActivityStatistics {
            method_version: 1,
            observed_use: activity_counts.0,
            no_observed_use: activity_counts.1,
            unavailable: activity_counts.2,
            scope: scope.clone(),
            checked_at: view.checked.clone(),
            items: extension_activity_items(&result.items, scope, &view.checked)?,
        };
        if r.action == Action::Detail {
            activity.observed_use = activity
                .items
                .iter()
                .filter(|i| i.observed_records.is_some_and(|n| n > 0))
                .count();
            activity.no_observed_use = activity
                .items
                .iter()
                .filter(|i| i.observed_records == Some(0))
                .count();
            activity.unavailable = activity
                .items
                .iter()
                .filter(|i| i.observed_records.is_none())
                .count();
        }
        result.extension_activity = Some(activity);
    }
    Ok(result)
}
