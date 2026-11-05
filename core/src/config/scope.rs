//! Configuration scope and ledger attribution filters.
use super::*;
pub(crate) fn validate(r: &Request) -> Result<()> {
    if r.limit.is_some_and(|n| n == 0 || n > 200)
        || r.offset.is_some_and(|n| n > MAX_SAFE_INTEGER as usize)
        || r.search.as_ref().is_some_and(|s| s.len() > 256)
        || r.roots.as_ref().is_some_and(|v| v.len() > 64)
        || r.project_roots.as_ref().is_some_and(|v| v.len() > 64)
    {
        return Err(operation_error("INVALID_ARGUMENT", "配置查询超出范围"));
    }
    if matches!(
        r.action,
        Action::Detail | Action::Evidence | Action::RelatedScopes
    ) && r.item_id.is_none()
    {
        return Err(operation_error("INVALID_ARGUMENT", "配置身份不能为空"));
    }
    if r.read_view.is_some() && r.snapshot_id.is_some() {
        return Err(operation_error("INVALID_ARGUMENT", "读取版本不能混用"));
    }
    validate_scope(&r.scope)?;
    if (r.unassigned || r.baseline.is_some() || r.baseline_records.is_some())
        && r.action != Action::Explain
        || r.unassigned && r.item_id.is_some()
        || r.baseline_records.is_some() && r.baseline.is_none()
        || r.baseline_records
            .as_ref()
            .is_some_and(|records| records.len() > 200)
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "unassigned/baseline 仅用于解释查询，unassigned 不能与 itemId 混用",
        ));
    }
    if r.action == Action::Explain
        && (r.observation.is_some() || r.kinds.is_some() || r.search.is_some())
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "explain 不支持 observation/kinds/search，请使用 itemId/kind 选择对象",
        ));
    }
    if let Some(baseline) = &r.baseline {
        if baseline.format_version != 1 {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此计数解释基线 formatVersion",
            ));
        }
        if baseline.reasons.len() > 14
            || baseline.roots.len() > 64
            || baseline.project_roots.len() > 64
            || baseline.counted > MAX_SAFE_INTEGER
            || baseline.total_records > MAX_SAFE_INTEGER as usize
            || [
                &baseline.input_revision,
                &baseline.configuration_revision,
                &baseline.result_revision,
            ]
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "计数解释基线超出范围或版本指纹无效",
            ));
        }
        validate_scope(&baseline.scope)?;
    }
    Ok(())
}
fn validate_scope(scope: &Scope) -> Result<Tz> {
    let tz: Tz = scope
        .timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| operation_error("INVALID_ARGUMENT", "时区无效"))?;
    if scope.all_time == Some(true) && (scope.since.is_some() || scope.until.is_some()) {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "全部日期不能与日期范围混用",
        ));
    }
    let date = |value: &Option<String>| -> Result<Option<NaiveDate>> {
        value
            .as_deref()
            .map(|value| {
                NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| operation_error("INVALID_ARGUMENT", "日期无效"))
            })
            .transpose()
    };
    if let (Some(since), Some(until)) = (date(&scope.since)?, date(&scope.until)?)
        && since >= until
    {
        return Err(operation_error("INVALID_ARGUMENT", "日期范围无效"));
    }
    Ok(tz)
}
/// Defaults belong to the selected view, including requests specifying only one date.
/// Validate the captured clock even for all-time reads; malformed views fail closed.
pub(super) fn normalize_at(scope: &Scope, checked: &str) -> Result<(Scope, Tz)> {
    let cutoff = DateTime::parse_from_rfc3339(checked)
        .map_err(|_| operation_error("INVALID_FACTS", "配置观察截止时间无效"))?;
    let tz = validate_scope(scope)?;
    let today = cutoff.with_timezone(&tz).date_naive();
    if scope.all_time == Some(true) {
        let mut out = scope.clone();
        out.timezone = Some(tz.to_string());
        return Ok((out, tz));
    }
    let parse = |s: &Option<String>, fallback: NaiveDate| -> Result<NaiveDate> {
        match s {
            Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| operation_error("INVALID_ARGUMENT", "日期无效")),
            None => Ok(fallback),
        }
    };
    let since_default = today
        .checked_sub_signed(chrono::Duration::days(29))
        .ok_or_else(|| operation_error("INVALID_FACTS", "配置观察日期超出范围"))?;
    let until_default = today
        .checked_add_signed(chrono::Duration::days(1))
        .ok_or_else(|| operation_error("INVALID_FACTS", "配置观察日期超出范围"))?;
    let since = parse(&scope.since, since_default)?;
    let until = parse(&scope.until, until_default)?;
    if since >= until {
        return Err(operation_error("INVALID_ARGUMENT", "日期范围无效"));
    }
    let mut out = scope.clone();
    out.timezone = Some(tz.to_string());
    out.since = Some(since.to_string());
    out.until = Some(until.to_string());
    Ok((out, tz))
}
#[cfg(test)]
pub(super) fn normalize(scope: &Scope) -> Result<(Scope, Tz)> {
    normalize_at(scope, "2000-01-01T00:00:00Z")
}
pub(super) fn in_time(at: Option<&str>, scope: &Scope, tz: Tz) -> bool {
    if scope.all_time == Some(true) {
        return true;
    }
    at.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|at| {
            let date = at.with_timezone(&tz).date_naive().to_string();
            date >= *scope.since.as_ref().unwrap() && date < *scope.until.as_ref().unwrap()
        })
}
pub(super) fn applicable(item: &Item, scope: &Scope) -> bool {
    scope.agent_kind.as_deref().is_none_or(|a| a == "codex")
        && item.applies(
            scope.source_instance_id.as_deref(),
            scope.project.as_deref(),
        )
}

pub(super) fn matches_row(
    row: &PricedMeasurement,
    scope: &Scope,
    tz: Tz,
    projects: &BTreeMap<&str, Option<&str>>,
) -> bool {
    let f = &row.fact;
    in_time(f.timestamp.as_deref(), scope, tz)
        && scope
            .agent_kind
            .as_ref()
            .is_none_or(|a| a.as_str() == f.agent_kind.as_ref())
        && scope
            .source_instance_id
            .as_ref()
            .is_none_or(|s| s.as_str() == f.source_instance_id.as_ref())
        && scope
            .thread_id
            .as_ref()
            .is_none_or(|s| f.thread_id.as_deref() == Some(s.as_str()))
        && scope.project.as_ref().is_none_or(|p| {
            f.thread_id
                .as_deref()
                .and_then(|id| projects.get(id).copied().flatten())
                == Some(p.as_str())
        })
}
pub(super) fn usage(
    rows: &[&PricedMeasurement],
) -> Result<Option<crate::usage_app_dto::UsageSummary>> {
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(crate::usage_app::summarize(rows)?))
    }
}

pub(crate) fn projects(r: &Request) -> Result<Vec<String>> {
    let mut roots = r.project_roots.clone().unwrap_or_default();
    roots.extend(crate::directories::authorized(
        crate::directories::Purpose::Project,
    )?);
    let mut out = BTreeSet::new();
    for root in roots {
        out.insert(crate::directories::canonical_project(&root)?);
    }
    Ok(out.into_iter().collect())
}
