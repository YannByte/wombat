//! Only explicit user decisions are durable. Inspection and rechecks never write here.
use crate::{dto::operation_error, optimize_dto::*};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use std::path::Path;
const VERSION: i64 = 4;
const SCHEMA: &str = "CREATE TABLE review_parts(id INTEGER PRIMARY KEY, hash TEXT NOT NULL UNIQUE, payload BLOB NOT NULL);
CREATE TABLE review_events(seq INTEGER PRIMARY KEY AUTOINCREMENT, object_id TEXT NOT NULL, suggestion_id TEXT NOT NULL, scope_project TEXT, status TEXT NOT NULL, decided INTEGER NOT NULL, kind TEXT NOT NULL, category TEXT NOT NULL, basis INTEGER NOT NULL REFERENCES review_parts(id), item INTEGER NOT NULL REFERENCES review_parts(id), baseline INTEGER REFERENCES review_parts(id), checks INTEGER NOT NULL REFERENCES review_parts(id), event BLOB NOT NULL);
CREATE INDEX review_scope_seq ON review_events(scope_project,seq DESC);
CREATE INDEX review_scope_suggestion ON review_events(scope_project,suggestion_id,seq DESC);
CREATE INDEX review_scope_object ON review_events(scope_project,object_id,seq DESC);";
fn open(path: &Path, write: bool) -> Result<Option<Connection>> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !write => return Ok(None),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    if write {
        crate::storage::private_file(path)?;
    }
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(operation_error(
            "REVIEWS_UNAVAILABLE",
            "Decision store is not a regular file",
        ));
    }
    let flags = if write {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let mut db = Connection::open_with_flags(
        path.parent()
            .unwrap()
            .canonicalize()?
            .join(path.file_name().unwrap()),
        flags | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(std::time::Duration::from_millis(500))?;
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 && write {
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%')",[],|r|r.get(0))?;
        if occupied {
            return Err(operation_error(
                "REVIEWS_UNAVAILABLE",
                "Unknown decision store; data retained",
            ));
        }
        tx.execute_batch(SCHEMA)?;
        tx.pragma_update(None, "user_version", VERSION)?;
        tx.commit()?;
    } else if version != VERSION {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported decision store; data retained",
        ));
    }
    Ok(Some(db))
}
fn revision(db: &Connection) -> Result<String> {
    Ok(db
        .query_row("SELECT COALESCE(MAX(seq),0) FROM review_events", [], |r| {
            r.get::<_, i64>(0)
        })?
        .to_string())
}
pub(super) fn read(
    path: &Path,
    suggestions: &mut Vec<Suggestion>,
    project: Option<&str>,
) -> Result<String> {
    let Some(db) = open(path, false)? else {
        return Ok("0".into());
    };
    let tx = db.unchecked_transaction()?;
    let revision = revision(&tx)?;
    let mut query=tx.prepare("SELECT json(e.event),json(b.payload) FROM review_events e JOIN review_parts b ON b.id=e.basis WHERE e.suggestion_id=?1 AND e.scope_project IS ?2 ORDER BY e.seq DESC LIMIT 1")?;
    let mut retained = Vec::with_capacity(suggestions.len());
    for mut s in suggestions.drain(..) {
        let saved: Option<(String, String)> = query
            .query_row(params![s.id, project], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((event, basis)) = saved {
            let basis: Value = serde_json::from_str(&basis)?;
            if basis["reviewFormatVersion"].as_u64() != Some(1) {
                return Err(operation_error(
                    "UNSUPPORTED_VERSION",
                    "Unsupported decision evidence; data retained",
                ));
            }
            let event: Value = serde_json::from_str(&event)?;
            if !event["decision"].is_null() {
                let decision: UserDecision = serde_json::from_value(event["decision"].clone())
                    .map_err(|_| {
                        operation_error("REVIEWS_UNAVAILABLE", "Invalid decision evidence")
                    })?;
                if decision.binding.version != 1 {
                    return Err(operation_error(
                        "UNSUPPORTED_VERSION",
                        "Unsupported decision binding; data retained",
                    ));
                }
                if super::identity::decision_applies(&decision, &s) {
                    s.decision = Some(decision);
                }
            }
        }
        if s.decision.is_none() {
            retained.push(s);
        }
    }
    *suggestions = retained;
    Ok(revision)
}
fn part(tx: &Transaction<'_>, value: &Value) -> Result<i64> {
    let text = serde_json::to_string(value)?;
    if text.len() > 2 * 1024 * 1024 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Decision evidence too large",
        ));
    }
    let hash = crate::hash(&text);
    tx.execute(
        "INSERT OR IGNORE INTO review_parts(hash,payload) VALUES(?1,jsonb(?2))",
        params![hash, text],
    )?;
    let (id, saved): (i64, String) = tx.query_row(
        "SELECT id,json(payload) FROM review_parts WHERE hash=?1",
        [hash],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if saved != text {
        return Err(operation_error(
            "REVIEWS_UNAVAILABLE",
            "Decision evidence identity conflict",
        ));
    }
    Ok(id)
}
pub(super) fn decide(path: &Path, r: &Request, s: &Suggestion) -> Result<()> {
    let expected = r
        .decision_revision
        .as_ref()
        .ok_or_else(|| operation_error("INVALID_ARGUMENT", "Decision revision is required"))?;
    if !path.exists() && expected != "0" {
        return Err(operation_error(
            "DECISION_CHANGED",
            "Decision revision changed",
        ));
    }
    let reason = r
        .decision_reason
        .clone()
        .ok_or_else(|| operation_error("INVALID_ARGUMENT", "A decision reason is required"))?;
    let kind = match (&r.action, &reason) {
        (Action::Keep, DecisionReason::Necessary) => DecisionKind::Keep,
        (
            Action::NotApplicable,
            DecisionReason::ObjectChanged | DecisionReason::IncorrectEvidence,
        ) => DecisionKind::NotApplicable,
        _ => {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Invalid decision reason",
            ));
        }
    };
    let binding = super::identity::binding(s);
    if binding.applicability_id.is_none() {
        return Err(operation_error(
            "EVIDENCE_UNAVAILABLE",
            "Decision requires complete problem evidence",
        ));
    }
    let mut db = open(path, true)?.unwrap();
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if r.decision_revision.as_ref() != Some(&revision(&tx)?) {
        return Err(operation_error(
            "DECISION_CHANGED",
            "Decision revision changed; reload the check",
        ));
    }
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))?;
    if count >= 20_000 {
        return Err(operation_error("RESOURCE_LIMIT", "Decision store is full"));
    }
    let decision = UserDecision {
        kind,
        reason,
        binding,
        recorded_at: chrono::Utc::now().to_rfc3339(),
    };
    let mut basis = serde_json::to_value(s)?;
    let object = basis.as_object_mut().unwrap();
    object.remove("baselineId");
    object.remove("checkedAt");
    let item = part(&tx, &object.remove("item").unwrap())?;
    let checks = part(&tx, &object.remove("checks").unwrap())?;
    object.remove("decision");
    let basis = part(&tx, &basis)?;
    let scope = &s
        .checks
        .first()
        .ok_or_else(|| operation_error("EVIDENCE_UNAVAILABLE", "Decision check is unavailable"))?
        .basis
        .scope;
    let baseline = part(
        &tx,
        &serde_json::json!({"version":1,"item":s.item,"scope":scope,"assessments":s.checks}),
    )?;
    let category = match s.category {
        Category::Repair => "repair",
        Category::Trim => "trim",
        Category::Review => "review",
    };
    let status = if r.action == Action::Keep {
        "kept"
    } else {
        "notApplicable"
    };
    let event = serde_json::json!({"decision":decision,"status":status,"checkedAt":s.checked_at,"recordId":uuid::Uuid::new_v4().to_string(),"recordedAt":chrono::Utc::now().to_rfc3339(),"recordKind":"decision","recheckRuleParameters":null});
    tx.execute("INSERT INTO review_events(object_id,suggestion_id,scope_project,status,decided,kind,category,basis,item,baseline,checks,event) VALUES(?1,?2,?3,?4,1,'decision',?5,?6,?7,?8,?9,jsonb(?10))",params![s.item.id,s.id,r.project,status,category,basis,item,baseline,checks,event.to_string()])?;
    tx.commit()?;
    Ok(())
}
