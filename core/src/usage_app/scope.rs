//! Query validation, timezones, filtering and coverage.
use super::*;
pub(super) fn invalid(message: impl Into<String>) -> anyhow::Error {
    operation_error("INVALID_ARGUMENT", message)
}
pub(crate) fn date(value: &str) -> Result<NaiveDate> {
    let result =
        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid("日期须为 YYYY-MM-DD"))?;
    if result.format("%Y-%m-%d").to_string() != value {
        return Err(invalid("日期须为 YYYY-MM-DD"));
    }
    Ok(result)
}
pub(crate) fn timezone(scope: &Scope) -> Result<Tz> {
    scope
        .timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| invalid("无效时区"))
}
pub(super) fn local_date(row: &PricedMeasurement, tz: Tz) -> Option<NaiveDate> {
    ObservedTime::parse(row).local_date(tz)
}
#[derive(Clone, Copy)]
pub(super) enum ObservedTime {
    Instant(DateTime<chrono::FixedOffset>),
    Date(NaiveDate),
    Unknown,
}
impl ObservedTime {
    pub(super) fn parse(row: &PricedMeasurement) -> Self {
        let Some(timestamp) = row.fact.timestamp.as_ref() else {
            return Self::Unknown;
        };
        if let Ok(value) = DateTime::parse_from_rfc3339(timestamp) {
            Self::Instant(value)
        } else if row.fact.time_precision.as_ref() == "date" {
            date(timestamp).map(Self::Date).unwrap_or(Self::Unknown)
        } else {
            Self::Unknown
        }
    }
    pub(super) fn local_date(self, tz: Tz) -> Option<NaiveDate> {
        match self {
            Self::Instant(value) => Some(value.with_timezone(&tz).date_naive()),
            Self::Date(value) => Some(value),
            Self::Unknown => None,
        }
    }
}
pub(super) fn dimensions(rows: &[&PricedMeasurement]) -> (Vec<String>, Vec<String>) {
    (
        rows.iter()
            .filter_map(|r| r.fact.model.raw.as_deref())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        rows.iter()
            .filter_map(|r| r.fact.reasoning_effort.as_deref())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect(),
    )
}
pub(super) fn matches(
    row: &PricedMeasurement,
    scope: &Scope,
    tz: Tz,
    projects: &BTreeMap<&str, Option<&str>>,
) -> bool {
    matches_using_date(row, scope, projects, || local_date(row, tz))
}
pub(super) fn matches_view(
    snapshot: &Snapshot,
    row: &PricedMeasurement,
    scope: &Scope,
    tz: Tz,
    projects: &BTreeMap<&str, Option<&str>>,
) -> bool {
    matches_using_date(row, scope, projects, || {
        snapshot.summary_index.local_date(row, tz)
    })
}
fn matches_using_date(
    row: &PricedMeasurement,
    scope: &Scope,
    projects: &BTreeMap<&str, Option<&str>>,
    date: impl FnOnce() -> Option<NaiveDate>,
) -> bool {
    let r = &row.fact;
    matches_fields(
        Fields {
            agent: &r.agent_kind,
            source: &r.source_instance_id,
            thread: r.thread_id.as_deref(),
            turn: r.turn_id.as_deref(),
            model: r.model.raw.as_deref(),
            effort: r.reasoning_effort.as_deref(),
        },
        scope,
        projects,
        date,
    )
}
pub(crate) struct Fields<'a> {
    pub agent: &'a str,
    pub source: &'a str,
    pub thread: Option<&'a str>,
    pub turn: Option<&'a str>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}
pub(crate) fn matches_fields(
    r: Fields<'_>,
    scope: &Scope,
    projects: &BTreeMap<&str, Option<&str>>,
    date: impl FnOnce() -> Option<NaiveDate>,
) -> bool {
    if scope
        .turn_id
        .as_ref()
        .is_some_and(|id| r.turn != Some(id.as_str()))
    {
        return false;
    }
    if scope
        .agent_kind
        .as_ref()
        .is_some_and(|v| v.as_str() != r.agent)
        || scope
            .source_instance_id
            .as_ref()
            .is_some_and(|v| v.as_str() != r.source)
        || scope
            .thread_id
            .as_ref()
            .is_some_and(|v| r.thread != Some(v.as_str()))
        || scope
            .model
            .as_ref()
            .is_some_and(|v| r.model != Some(v.as_str()))
        || scope
            .reasoning_effort
            .as_ref()
            .is_some_and(|v| r.effort != Some(v.as_str()))
    {
        return false;
    }
    if scope.model_unknown == Some(true) && r.model.is_some()
        || scope.effort_unknown == Some(true) && r.effort.is_some()
    {
        return false;
    }
    if scope.project_unknown == Some(true)
        && r.thread
            .and_then(|id| projects.get(id))
            .copied()
            .flatten()
            .is_some()
    {
        return false;
    }
    if scope.project.as_ref().is_some_and(|v| {
        r.thread.and_then(|id| projects.get(id)).copied().flatten() != Some(v.as_str())
    }) {
        return false;
    }
    // Dimension-only filters include dated and undated facts alike. Parsing and
    // formatting every timestamp here does not affect that selection.
    if scope.undated != Some(true) && scope.since.is_none() && scope.until.is_none() {
        return true;
    }
    let local = date().map(|d| d.to_string());
    if scope.undated == Some(true) {
        return local.is_none();
    }
    if scope
        .since
        .as_ref()
        .is_some_and(|since| local.as_ref().is_none_or(|d| d < since))
        || scope
            .until
            .as_ref()
            .is_some_and(|until| local.as_ref().is_none_or(|d| d >= until))
    {
        return false;
    }
    true
}
pub(super) fn available(rows: &[&PricedMeasurement], tz: Tz) -> AvailableRange {
    available_dates(rows.iter().filter_map(|r| local_date(r, tz)))
}
pub(super) fn available_view(
    snapshot: &Snapshot,
    rows: &[&PricedMeasurement],
    tz: Tz,
) -> AvailableRange {
    available_dates(
        rows.iter()
            .filter_map(|r| snapshot.summary_index.local_date(r, tz)),
    )
}
fn available_dates(dates: impl Iterator<Item = NaiveDate>) -> AvailableRange {
    let mut first = None;
    let mut last = None;
    for date in dates {
        first = Some(first.map_or(date, |old: NaiveDate| old.min(date)));
        last = Some(last.map_or(date, |old: NaiveDate| old.max(date)));
    }
    AvailableRange {
        since: first.map(|d| d.to_string()),
        until: last.and_then(|d| d.succ_opt()).map(|d| d.to_string()),
    }
}

pub(super) fn quality_status(snapshot: &Snapshot, count: usize) -> Quality {
    let sources = &snapshot.manifest.sources;
    let partial = !snapshot.manifest.issues.is_empty()
        || sources
            .iter()
            .any(|s| matches!(s.status.as_str(), "partial" | "failed" | "cancelled"));
    Quality {
        status: if partial {
            "partial"
        } else if count == 0 {
            "empty"
        } else {
            "complete"
        }
        .into(),
        issues: vec![],
        sources: vec![],
        detail_summary: None,
    }
}
pub(super) fn quality(snapshot: &Snapshot, count: usize) -> Quality {
    let mut result = quality_status(snapshot, count);
    result.issues = snapshot.manifest.issues.clone();
    result.sources = snapshot.manifest.sources.clone();
    result
}
pub(crate) fn validate(request: &Request) -> Result<()> {
    timezone(&request.scope)?;
    super::comparison::validate_comparison(request)?;
    super::statistics::validate(request)?;
    super::inspection::validate(request)?;
    if request
        .scope
        .turn_id
        .as_ref()
        .is_some_and(|id| id.is_empty())
        || request.scope.turn_id.is_some()
            && (request.action != Action::Investigate
                || request
                    .scope
                    .thread_id
                    .as_ref()
                    .or(request.thread_id.as_ref())
                    .is_none_or(|id| id.is_empty()))
    {
        return Err(invalid("轮次检查需要investigate和完整threadId"));
    }
    if request.scope.all_time == Some(true)
        && (request.scope.since.is_some()
            || request.scope.until.is_some()
            || request.scope.undated == Some(true))
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "全部日期不能与日期范围混用",
        ));
    }
    if let Some(d) = &request.scope.since {
        date(d)?;
    }
    if let Some(d) = &request.scope.until {
        date(d)?;
    }
    if request
        .scope
        .since
        .as_ref()
        .zip(request.scope.until.as_ref())
        .is_some_and(|(a, b)| a >= b)
    {
        return Err(invalid("起始日期必须早于结束日期（不含）"));
    }
    if request.limit.is_some_and(|n| n == 0 || n > 500)
        || request
            .offset
            .is_some_and(|n| n > MAX_SAFE_INTEGER as usize)
    {
        return Err(invalid("分页limit须为1至500，offset为安全非负整数"));
    }
    if request.roots.is_some() && request.action != Action::Refresh {
        return Err(invalid("来源根仅适用于更新"));
    }
    if request.action == Action::Refresh
        && (request.compact == Some(true)
            || request.snapshot_id.is_some()
            || request.thread_id.is_some()
            || request.turn_id.is_some()
            || request.group.is_some()
            || request.sort.is_some()
            || request.presentation.is_some()
            || request.search.is_some()
            || request.offset.is_some()
            || request.limit.is_some()
            || request.locate_thread_id.is_some()
            || request.locate_turn_id.is_some()
            || request.locate_operation_id.is_some()
            || request.matched_only.is_some()
            || serde_json::to_value(&request.scope)?
                .as_object()
                .is_some_and(|o| o.values().any(|v| !v.is_null())))
    {
        return Err(invalid("更新不接受查询筛选参数"));
    }
    if request.group.is_some() && request.action != Action::Usage {
        return Err(invalid("分组仅适用于用量"));
    }
    if request.presentation.is_some()
        && !matches!(request.action, Action::Usage | Action::Statistics)
    {
        return Err(invalid("展示分组仅适用于用量"));
    }
    if request.locate_operation_id.is_some() && request.action != Action::Steps {
        return Err(invalid("操作定位仅适用于轮次记录"));
    }
    if request.locate_thread_id.is_some() && request.action != Action::Threads {
        return Err(invalid("定位仅适用于对话"));
    }
    if (request.locate_turn_id.is_some() || request.matched_only.is_some())
        && request.action != Action::Turns
    {
        return Err(invalid("轮次定位和匹配筛选仅适用于轮次"));
    }
    if request.scope.project_unknown == Some(true) && request.scope.project.is_some() {
        return Err(invalid("未知项目与具体项目冲突"));
    }
    if request.search.is_some() && request.action != Action::Threads {
        return Err(invalid("搜索仅适用于对话"));
    }
    if matches!(request.action, Action::Turns | Action::Steps)
        && request.thread_id.as_deref().is_none_or(str::is_empty)
    {
        return Err(invalid("需要threadId"));
    }
    if request.action == Action::Steps && request.turn_id.as_deref().is_none_or(str::is_empty) {
        return Err(invalid("需要turnId"));
    }
    if request.turn_id.is_some() && request.action != Action::Steps {
        return Err(invalid("turnId仅适用于步骤"));
    }
    if matches!(request.sort, Some(Sort::Recent))
        && !matches!(request.action, Action::Threads | Action::Turns)
    {
        return Err(invalid("recent排序仅适用于对话或轮次"));
    }
    if matches!(request.sort, Some(Sort::Time)) && request.action == Action::Threads {
        return Err(invalid("对话排序使用tokens或recent"));
    }
    if request.scope.undated == Some(true)
        && (request.scope.since.is_some() || request.scope.until.is_some())
    {
        return Err(invalid("未知日期不能同时指定日期范围"));
    }
    if request.scope.model_unknown == Some(true) && request.scope.model.is_some()
        || request.scope.effort_unknown == Some(true) && request.scope.reasoning_effort.is_some()
    {
        return Err(invalid("未知维度与具体值冲突"));
    }
    Ok(())
}

/// A scoped query waits only for the selected recorded project, never unrelated loads.
pub(super) fn ensure_ready(request: &Request, snapshot: &Snapshot) -> Result<()> {
    let thread = request
        .scope
        .thread_id
        .as_ref()
        .or(request.thread_id.as_ref());
    let recorded_project = thread
        .and_then(|id| {
            snapshot
                .manifest
                .threads
                .iter()
                .find(|t| &t.thread.id == id)
        })
        .map(|t| t.thread.project.as_ref());
    if snapshot.project_loads.iter().any(|load| {
        load.state != crate::live::ProjectLoadState::Ready
            && if let Some(project) = &request.scope.project {
                load.project.as_ref() == Some(project)
            } else if request.scope.project_unknown == Some(true) {
                load.project.is_none()
            } else if let Some(project) = recorded_project {
                load.project.as_ref() == project
            } else {
                super::inspection::is_action(&request.action)
                    || request.action == Action::Statistics
            }
    }) {
        return Err(operation_error(
            "SYNC_PENDING",
            "所选范围正在加载，已加载的项目可以查看",
        ));
    }
    Ok(())
}
