//! User-owned budgets and periodic review notifications. Hosts explicitly drive checks.
pub const REQUEST_VERSION: u32 = 2;

use crate::{
    dto::operation_error,
    usage_app_dto::{Scope, SnapshotRef, TaskStatistics, UsageSummary},
    usage_store::Snapshot,
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
mod store;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Day,
    Week,
    Month,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    #[schemars(length(min = 1, max = 64))]
    pub id: String,
    pub enabled: bool,
    pub period: Period,
    /// Identity filters and timezone only; date, undated, thread and turn filters are rejected.
    #[serde(default)]
    pub scope: Scope,
    /// Token budget, not an account allowance or money allocation to tools.
    pub token_limit: Option<u64>,
    /// Fraction in (0,1]; 0.8 by default.
    pub warning_ratio: Option<f64>,
    pub review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    List,
    Upsert {
        plan: Box<Plan>,
    },
    Remove {
        id: String,
    },
    Check {
        #[serde(default)]
        roots: Vec<String>,
        #[serde(default, skip_serializing)]
        #[schemars(skip)]
        snapshot_id: String,
        ids: Vec<String>,
    },
    Acknowledge {
        notification_id: String,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    List,
    Upsert,
    Remove,
    Check,
    Acknowledge,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    BudgetWarning,
    BudgetExceeded,
    PeriodReview,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Authorized source set used for these facts and notification deduplication.
    pub source_instance_ids: Vec<String>,
    pub id: String,
    pub plan_id: String,
    pub plan_revision: String,
    pub kind: NotificationKind,
    pub checked_at: String,
    pub snapshot_ref: SnapshotRef,
    pub scope: Scope,
    pub summary: UsageSummary,
    pub statistics: TaskStatistics,
    pub partial: bool,
    pub acknowledged: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub action: Action,
    pub snapshot_id: Option<String>,
    #[schemars(range(min = 1, max = 1))]
    pub output_version: u32,
    pub plans: Vec<Plan>,
    /// Most recent 100 notifications. New checks return only newly created notifications.
    pub notifications: Vec<Notification>,
    pub checked_at: String,
    pub host_required: bool,
}
fn invalid() -> anyhow::Error {
    operation_error("INVALID_ARGUMENT", "Invalid monitor request")
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn validate(plan: &Plan) -> Result<()> {
    let scope = &plan.scope;
    if !valid_id(&plan.id)
        || plan
            .token_limit
            .is_some_and(|v| v == 0 || v > crate::adapters::contract::MAX_SAFE_INTEGER)
        || plan
            .warning_ratio
            .is_some_and(|v| !v.is_finite() || v <= 0.0 || v > 1.0)
        || plan.token_limit.is_none() && !plan.review
        || scope.since.is_some()
        || scope.until.is_some()
        || scope.all_time.is_some()
        || scope.undated.is_some()
        || scope.thread_id.is_some()
        || scope.turn_id.is_some()
    {
        return Err(invalid());
    }
    let r: crate::usage_app_dto::Request =
        serde_json::from_value(serde_json::json!({"action":"statistics","scope":scope}))?;
    crate::usage_app::validate(&r)?;
    Ok(())
}
fn bounds(date: NaiveDate, period: &Period) -> Result<(NaiveDate, NaiveDate, NaiveDate)> {
    let start = match period {
        Period::Day => Some(date),
        Period::Week => {
            date.checked_sub_signed(Duration::days(date.weekday().num_days_from_monday() as i64))
        }
        Period::Month => date.with_day(1),
    };
    let start = start.ok_or_else(invalid)?;
    let (previous, end) = match period {
        Period::Day => (
            start.checked_sub_signed(Duration::days(1)),
            start.succ_opt(),
        ),
        Period::Week => (
            start.checked_sub_signed(Duration::days(7)),
            start.checked_add_signed(Duration::days(7)),
        ),
        Period::Month => (
            start.checked_sub_months(Months::new(1)),
            start.checked_add_months(Months::new(1)),
        ),
    };
    Ok((
        previous.ok_or_else(invalid)?,
        start,
        end.ok_or_else(invalid)?,
    ))
}
fn plans(db: &Connection) -> Result<Vec<Plan>> {
    let mut statement = db.prepare("SELECT payload FROM plans ORDER BY id LIMIT 101")?;
    let values = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.len() > 100 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Monitor plan limit exceeded",
        ));
    }
    values
        .into_iter()
        .map(|value| {
            if value.len() > 16_384 {
                return Err(operation_error("RESOURCE_LIMIT", "Monitor plan too large"));
            }
            let p: Plan = serde_json::from_str(&value)?;
            validate(&p)?;
            Ok(p)
        })
        .collect()
}
fn evaluate(plan: &Plan, snapshot: &Snapshot, at: DateTime<Utc>) -> Result<Vec<Notification>> {
    if plan.scope.source_instance_id.as_ref().is_some_and(|id| {
        !snapshot
            .manifest
            .sources
            .iter()
            .any(|report| &report.source.id == id)
    }) {
        return Err(operation_error(
            "SOURCE_NOT_AUTHORIZED",
            "Monitor source is outside this view",
        ));
    }
    let tz: Tz = plan
        .scope
        .timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| invalid())?;
    let (previous, start, end) = bounds(at.with_timezone(&tz).date_naive(), &plan.period)?;
    let revision = crate::hash(serde_json::to_vec(plan)?);
    let query = |since: NaiveDate, until: NaiveDate| -> Result<crate::usage_app_dto::Response> {
        let mut scope = plan.scope.clone();
        scope.since = Some(since.to_string());
        scope.until = Some(until.to_string());
        let request = serde_json::from_value(
            serde_json::json!({"action":"statistics","scope":scope,"compact":true}),
        )?;
        crate::usage_app::execute_snapshot(request, snapshot)
    };
    let notify = |kind: NotificationKind, result: crate::usage_app_dto::Response| -> Notification {
        let sources = snapshot
            .manifest
            .sources
            .iter()
            .filter(|source| {
                plan.scope
                    .source_instance_id
                    .as_ref()
                    .is_none_or(|id| &source.source.id == id)
            })
            .collect::<Vec<_>>();
        let coverage = sources
            .iter()
            .map(|source| (&source.source.id, &source.status))
            .collect::<std::collections::BTreeSet<_>>();
        let review_revision = if matches!(kind, NotificationKind::PeriodReview) {
            Some(crate::hash(
                serde_json::to_vec(&(
                    &result.summary,
                    &result.statistics,
                    result.quality.status.as_str(),
                    &coverage,
                ))
                .expect("serializable review facts"),
            ))
        } else {
            None
        };
        let source_instance_ids = sources
            .iter()
            .filter(|source| {
                plan.scope
                    .source_instance_id
                    .as_ref()
                    .is_none_or(|id| &source.source.id == id)
            })
            .map(|source| source.source.id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let id = crate::hash(
            serde_json::to_vec(&(
                &source_instance_ids,
                &plan.id,
                &revision,
                &kind,
                &result.scope.since,
                &result.scope.until,
                review_revision,
            ))
            .expect("serializable notification identity"),
        );
        Notification {
            source_instance_ids,
            id,
            plan_id: plan.id.clone(),
            plan_revision: revision.clone(),
            kind,
            checked_at: at.to_rfc3339(),
            partial: sources.iter().any(|source| source.status != "complete")
                || result.quality.status == "partial"
                || result.summary.complete_token_total().is_none(),
            snapshot_ref: result.snapshot_ref,
            scope: result.scope,
            summary: result.summary,
            statistics: result.statistics.expect("statistics query"),
            acknowledged: false,
        }
    };
    let mut out = vec![];
    if let Some(limit) = plan.token_limit {
        let result = query(start, end)?;
        // A partial subtotal may prove crossing, never remaining allowance or absence of crossing.
        if let Some(tokens) = result.summary.available_token_subtotal() {
            if tokens >= limit {
                out.push(notify(NotificationKind::BudgetExceeded, result));
            } else if tokens as f64 >= limit as f64 * plan.warning_ratio.unwrap_or(0.8) {
                out.push(notify(NotificationKind::BudgetWarning, result));
            }
        }
    }
    if plan.review {
        out.push(notify(
            NotificationKind::PeriodReview,
            query(previous, start)?,
        ));
    }
    Ok(out)
}
pub fn dispatch(request: Request) -> Result<Response> {
    let mut db = store::open(&crate::storage::data_home()?.join("user-v1/monitor.sqlite"))?;
    dispatch_with(&mut db, request, Utc::now(), None)
}
pub(crate) fn dispatch_snapshot(
    request: Request,
    snapshot: &Snapshot,
    checked_at: Option<&str>,
) -> Result<Response> {
    let now = Utc::now();
    let checked = checked_at
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.with_timezone(&Utc));
    if checked.is_none_or(|v| now - v > Duration::minutes(5) || v - now > Duration::minutes(1)) {
        return Err(operation_error(
            "SOURCE_STALE",
            "Monitor needs a recent source check",
        ));
    }
    let mut db = store::open(&crate::storage::data_home()?.join("user-v1/monitor.sqlite"))?;
    dispatch_with(&mut db, request, now, Some(snapshot))
}
fn dispatch_with(
    db: &mut Connection,
    request: Request,
    at: DateTime<Utc>,
    fixed: Option<&Snapshot>,
) -> Result<Response> {
    let action = match &request {
        Request::List => Action::List,
        Request::Upsert { .. } => Action::Upsert,
        Request::Remove { .. } => Action::Remove,
        Request::Check { .. } => Action::Check,
        Request::Acknowledge { .. } => Action::Acknowledge,
    };
    let requested_snapshot = match &request {
        Request::Check { snapshot_id, .. } => Some(snapshot_id.clone()),
        _ => None,
    };
    let mut notifications = vec![];
    match request {
        Request::List => {
            let mut stmt = db.prepare(
                "SELECT payload,acknowledged FROM notifications ORDER BY seq DESC LIMIT 100",
            )?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))? {
                let (payload, ack) = row?;
                if payload.len() > 262_144 {
                    return Err(operation_error(
                        "RESOURCE_LIMIT",
                        "Monitor notification too large",
                    ));
                }
                let mut n: Notification = serde_json::from_str(&payload)?;
                n.acknowledged = ack;
                notifications.push(n);
            }
        }
        Request::Upsert { plan } => {
            validate(&plan)?;
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM plans WHERE id=?1)",
                [&plan.id],
                |r| r.get(0),
            )?;
            let count: u32 = tx.query_row("SELECT COUNT(*) FROM plans", [], |r| r.get(0))?;
            if !exists && count >= 100 {
                return Err(operation_error(
                    "RESOURCE_LIMIT",
                    "Monitor plan limit exceeded",
                ));
            }
            let payload = serde_json::to_string(&plan)?;
            if payload.len() > 16_384 {
                return Err(invalid());
            }
            tx.execute("INSERT INTO plans(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",params![plan.id,payload])?;
            tx.commit()?;
        }
        Request::Remove { id } => {
            if !valid_id(&id) {
                return Err(invalid());
            }
            db.execute("DELETE FROM plans WHERE id=?1", [id])?;
        }
        Request::Acknowledge { notification_id } => {
            if notification_id.len() != 64
                || !notification_id.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(invalid());
            }
            if db.execute(
                "UPDATE notifications SET acknowledged=1 WHERE id=?1",
                [notification_id],
            )? == 0
            {
                return Err(operation_error("NOT_FOUND", "Notification not found"));
            }
        }
        Request::Check {
            snapshot_id, ids, ..
        } => {
            if ids.is_empty()
                || ids.len() > 100
                || ids.iter().any(|id| !valid_id(id))
                || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
            {
                return Err(invalid());
            }
            let owned;
            let snapshot = if let Some(s) = fixed {
                s
            } else {
                owned = crate::usage_store::load(Some(&snapshot_id))?;
                &owned
            };
            if snapshot.manifest.snapshot_ref.snapshot_id != snapshot_id {
                return Err(invalid());
            }
            // Notifications require a recent current view, not a stale historical publication.
            let published =
                DateTime::parse_from_rfc3339(&snapshot.manifest.snapshot_ref.created_at)
                    .map_err(|_| invalid())?
                    .with_timezone(&Utc);
            if fixed.is_none()
                && (at - published > Duration::minutes(5) || published - at > Duration::minutes(1))
            {
                return Err(operation_error(
                    "SOURCE_STALE",
                    "Monitor needs a recent source observation",
                ));
            }
            let current = plans(db)?;
            for id in &ids {
                let plan = current
                    .iter()
                    .find(|p| &p.id == id)
                    .ok_or_else(|| operation_error("NOT_FOUND", "Monitor plan not found"))?;
                if plan.enabled {
                    notifications.extend(evaluate(plan, snapshot, at)?);
                }
            }
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let mut inserted = vec![];
            for n in notifications {
                // Revalidate a concurrently changed plan before publishing facts for it.
                let payload: Option<String> = tx
                    .query_row("SELECT payload FROM plans WHERE id=?1", [&n.plan_id], |r| {
                        r.get(0)
                    })
                    .optional()?;
                if payload
                    .as_ref()
                    .is_none_or(|p| crate::hash(p) != n.plan_revision)
                {
                    return Err(operation_error(
                        "MONITOR_CHANGED",
                        "Monitor settings changed; retry",
                    ));
                }
                let payload = serde_json::to_string(&n)?;
                if payload.len() > 262_144 {
                    return Err(operation_error(
                        "RESOURCE_LIMIT",
                        "Monitor notification too large",
                    ));
                }
                if tx.execute("INSERT OR IGNORE INTO notifications(id,plan_id,payload,acknowledged) VALUES(?1,?2,?3,0)",params![n.id,n.plan_id,payload])?>0 {inserted.push(n);}
            }
            tx.commit()?;
            notifications = inserted;
        }
    }
    Ok(Response {
        action,
        snapshot_id: requested_snapshot,
        output_version: 1,
        plans: plans(db)?,
        notifications,
        checked_at: at.to_rfc3339(),
        host_required: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_crossings_reviews_acknowledgements_and_disabled_plans_are_durable() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        let records = [
            serde_json::json!({"type":"session_meta","payload":{"id":"t","cwd":"/synthetic"}}),
            serde_json::json!({"type":"turn_context","payload":{"turn_id":"u","model":"gpt-5.4"}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-10-09T00:01:00Z","payload":{"type":"token_usage_record","thread_id":"t","turn_id":"u","response_id":"r","usage":{"input_tokens":100,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":10,"reasoning_output_tokens":0,"total_tokens":110}}}),
        ];
        std::fs::write(
            root.join("sessions/t.jsonl"),
            records.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        let collected = crate::adapters::collect(
            &crate::adapters::contract::DiscoveryRequest { roots: vec![root] },
            &Default::default(),
        );
        let snapshot = crate::usage_store::memory(
            collected,
            "live:synthetic:monitor".into(),
            crate::pricing_sync::current_at(temp.path()).unwrap(),
            None,
        )
        .unwrap();
        let mut db = store::open(&temp.path().join("monitor.sqlite")).unwrap();
        let at = DateTime::parse_from_rfc3339("2026-10-09T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut p:Plan=serde_json::from_value(serde_json::json!({"id":"daily","enabled":true,"period":"day","scope":{"timezone":"UTC"},"tokenLimit":100,"warningRatio":0.8,"review":true})).unwrap();
        dispatch_with(
            &mut db,
            Request::Upsert {
                plan: Box::new(p.clone()),
            },
            at,
            None,
        )
        .unwrap();
        let check = || Request::Check {
            roots: vec![],
            snapshot_id: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
            ids: vec!["daily".into()],
        };
        let first = dispatch_with(&mut db, check(), at, Some(&snapshot)).unwrap();
        assert_eq!(first.notifications.len(), 2);
        assert!(first.notifications.iter().any(|n| matches!(
            n.kind,
            NotificationKind::BudgetExceeded
        ) && n.summary.complete_token_total()
            == Some(110)));
        assert!(
            dispatch_with(&mut db, check(), at, Some(&snapshot))
                .unwrap()
                .notifications
                .is_empty()
        );
        let id = first.notifications[0].id.clone();
        dispatch_with(
            &mut db,
            Request::Acknowledge {
                notification_id: id.clone(),
            },
            at,
            None,
        )
        .unwrap();
        assert!(
            dispatch_with(&mut db, Request::List, at, None)
                .unwrap()
                .notifications
                .iter()
                .find(|n| n.id == id)
                .unwrap()
                .acknowledged
        );
        p.enabled = false;
        dispatch_with(&mut db, Request::Upsert { plan: Box::new(p) }, at, None).unwrap();
        assert!(
            dispatch_with(&mut db, check(), at + Duration::days(1), Some(&snapshot))
                .unwrap()
                .notifications
                .is_empty()
        );
        drop(db);
        let mut reopened = store::open(&temp.path().join("monitor.sqlite")).unwrap();
        assert_eq!(
            dispatch_with(&mut reopened, Request::List, at, None)
                .unwrap()
                .notifications
                .len(),
            2
        );
    }
    #[test]
    fn calendar_periods_use_local_dates_and_real_month_lengths() {
        let d = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
        assert_eq!(
            bounds(d, &Period::Week).unwrap(),
            (
                NaiveDate::from_ymd_opt(2024, 2, 26).unwrap(),
                NaiveDate::from_ymd_opt(2024, 3, 4).unwrap(),
                NaiveDate::from_ymd_opt(2024, 3, 11).unwrap()
            )
        );
        assert_eq!(
            bounds(d, &Period::Month).unwrap().0,
            NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()
        );
    }
    #[test]
    fn settings_persist_and_unknown_formats_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("monitor.sqlite");
        let mut db = store::open(&file).unwrap();
        let p:Plan=serde_json::from_value(serde_json::json!({"id":"weekly","enabled":true,"period":"week","review":true,"scope":{"project":"/synthetic"},"tokenLimit":1000})).unwrap();
        let r = dispatch_with(
            &mut db,
            Request::Upsert { plan: Box::new(p) },
            Utc::now(),
            None,
        )
        .unwrap();
        assert_eq!(r.plans.len(), 1);
        db.pragma_update(None, "user_version", 99).unwrap();
        drop(db);
        assert!(store::open(&file).is_err());
        let db = Connection::open(&file).unwrap();
        assert_eq!(plans(&db).unwrap().len(), 1);
    }
}
