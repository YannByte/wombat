//! Private runtime observations. Hooks never create ledger measurements or retain bodies.
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

mod association;
pub use association::{Association, AssociationState};

const EVENT_LIMIT: u64 = 100_000;
const BUFFER_LIMIT: u64 = 4096;
const INPUT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Logs,
    Hooks,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    Status,
    Events,
    Configure,
    Pause,
    Resume,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub action: Action,
    pub mode: Option<Mode>,
    pub roots: Option<Vec<String>>,
    pub project: Option<String>,
    pub source_instance_id: Option<String>,
    pub after: Option<u64>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum EventKind {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PermissionRequest,
    PreCompact,
    PostCompact,
    SubagentStart,
    SubagentStop,
    Stop,
    Interrupt,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub id: String,
    pub source_instance_id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub project: Option<String>,
    pub kind: EventKind,
    pub occurred_at: Option<String>,
    pub native_identity: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub sequence: u64,
    pub received_at: String,
    pub buffered: bool,
    pub observation: Observation,
    pub association: Association,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    LogsOnly,
    Waiting,
    Received,
    Paused,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub output_version: u32,
    pub action: Action,
    pub checked_at: String,
    pub mode: Mode,
    pub state: State,
    pub received: u64,
    pub buffered: u64,
    pub gaps: u64,
    pub identity_unknown: u64,
    pub last_received_at: Option<String>,
    pub events: Vec<EventRow>,
    pub next_after: Option<u64>,
    pub event_limit: u64,
    pub buffer_limit: u64,
}

fn count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(e),
        )
    })
}
fn directory() -> Result<PathBuf> {
    Ok(crate::storage::data_home()?.join("collection-v1"))
}
fn open(root: &Path) -> Result<Connection> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(root)?;
    let file = dunce::canonicalize(root)?.join("observations.sqlite");
    // Create private before SQLite opens it, and reject links/non-files on every entry.
    match std::fs::symlink_metadata(&file) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err(crate::dto::operation_error(
                "COLLECTION_UNAVAILABLE",
                "Unsafe collection file",
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&file) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        Err(e) => return Err(e.into()),
    }
    let mut db = Connection::open_with_flags(
        &file,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(std::time::Duration::from_millis(500))?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let version: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        let occupied: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%')", [], |r|r.get(0))?;
        if occupied {
            return Err(crate::dto::operation_error(
                "COLLECTION_UNSUPPORTED_VERSION",
                "Unknown collection format",
            ));
        }
        tx.execute_batch("CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK(id=1), mode TEXT NOT NULL, paused INTEGER NOT NULL); INSERT INTO settings VALUES(1,'logs',0); CREATE TABLE events (sequence INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE, source TEXT NOT NULL, project TEXT, received_at TEXT NOT NULL, buffered INTEGER NOT NULL, payload TEXT NOT NULL); CREATE INDEX event_scope ON events(source,project,sequence); CREATE INDEX buffered_events ON events(buffered) WHERE buffered=1; CREATE TABLE gaps (source TEXT NOT NULL, project TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY(source,project)); PRAGMA user_version=1;")?;
    } else if version != 1 {
        return Err(crate::dto::operation_error(
            "COLLECTION_UNSUPPORTED_VERSION",
            "Unknown collection format",
        ));
    }
    tx.commit()?;
    Ok(db)
}
fn text(body: &Value, key: &str) -> Option<String> {
    body.get(key)?
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 256
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
        })
        .map(str::to_owned)
}
fn safe_observation(body: &Value, source: &str) -> Result<Observation> {
    if !body.is_object() || serde_json::to_vec(body)?.len() > INPUT_LIMIT {
        return Err(crate::dto::operation_error(
            "INVALID_ARGUMENT",
            "Invalid Hook input",
        ));
    }
    let kind: EventKind =
        serde_json::from_value(body.get("hook_event_name").cloned().unwrap_or(Value::Null))
            .map_err(|_| {
                crate::dto::operation_error("INVALID_ARGUMENT", "Unsupported Hook event")
            })?;
    let session_id = text(body, "session_id").ok_or_else(|| {
        crate::dto::operation_error("INVALID_ARGUMENT", "Missing native session identity")
    })?;
    let turn_id = text(body, "turn_id");
    let tool_use_id = text(body, "tool_use_id");
    let agent_id = text(body, "agent_id");
    let native_id = text(body, "event_id").or_else(|| {
        matches!(
            kind,
            EventKind::PreToolUse | EventKind::PostToolUse | EventKind::PermissionRequest
        )
        .then(|| tool_use_id.clone())
        .flatten()
    });
    let project = body
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|s| {
            s.len() <= 4096 && !s.chars().any(char::is_control) && Path::new(s).is_absolute()
        })
        .map(str::to_owned);
    let occurred_at = body
        .get("timestamp")
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 64)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.to_utc().to_rfc3339());
    let id = if let Some(native) = &native_id {
        crate::hash(serde_json::to_vec(&(source, &session_id, &kind, native))?)
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    Ok(Observation {
        id,
        source_instance_id: source.into(),
        session_id,
        turn_id,
        tool_use_id,
        agent_id,
        project,
        kind,
        occurred_at,
        native_identity: native_id.is_some(),
    })
}
fn native_source() -> Result<String> {
    use crate::adapters::contract::AgentAdapter;
    let roots = crate::adapters::codex::CodexAdapter.discover(
        &crate::adapters::contract::DiscoveryRequest {
            roots: vec![
                crate::host_environment::get("CODEX_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| crate::home().join(".codex")),
            ],
        },
    );
    Ok(roots.sources[0].id.clone())
}
/// Raw Hook data is accepted only on the private local process boundary, never HTTP.
pub fn ingest(body: Value) -> Result<()> {
    ingest_at(
        &directory()?,
        safe_observation(&body, &native_source()?)?,
        EVENT_LIMIT,
        BUFFER_LIMIT,
    )
}
fn ingest_at(
    root: &Path,
    observation: Observation,
    event_limit: u64,
    buffer_limit: u64,
) -> Result<()> {
    let mut db = open(root)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let (mode, paused): (String, bool) =
        tx.query_row("SELECT mode,paused FROM settings WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    if mode == "logs" {
        tx.commit()?;
        return Ok(());
    }
    let payload = serde_json::to_string(&observation)?;
    if let Some(prior) = tx
        .query_row(
            "SELECT payload FROM events WHERE id=?1",
            [&observation.id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        if prior != payload {
            gap(&tx, &observation)?;
        }
        tx.commit()?;
        return Ok(());
    }
    let (total, buffered): (u64, u64) = tx.query_row(
        "SELECT COALESCE((SELECT MAX(sequence) FROM events),0),(SELECT COUNT(*) FROM events WHERE buffered=1)",
        [],
        |r| Ok((count(r, 0)?, count(r, 1)?)),
    )?;
    if total >= event_limit || paused && buffered >= buffer_limit {
        gap(&tx, &observation)?;
    } else {
        tx.execute("INSERT INTO events(id,source,project,received_at,buffered,payload) VALUES(?1,?2,?3,?4,?5,?6)",params![observation.id,observation.source_instance_id,observation.project,chrono::Utc::now().to_rfc3339(),paused,payload])?;
    }
    tx.commit()?;
    Ok(())
}
fn gap(db: &Connection, o: &Observation) -> Result<()> {
    db.execute("INSERT INTO gaps VALUES(?1,?2,1) ON CONFLICT(source,project) DO UPDATE SET count=MIN(count+1,9007199254740991)",params![o.source_instance_id,o.project.as_deref().unwrap_or("")])?;
    Ok(())
}
fn scope(r: &Request) -> Result<Vec<String>> {
    use crate::adapters::contract::AgentAdapter;
    if r.roots
        .as_ref()
        .is_some_and(|v| v.is_empty() || v.len() > 64)
    {
        return Err(crate::dto::operation_error(
            "INVALID_ARGUMENT",
            "Invalid collection roots",
        ));
    }
    let sources: Vec<String> = crate::adapters::codex::CodexAdapter
        .discover(&crate::adapters::contract::DiscoveryRequest {
            roots: r
                .roots
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        })
        .sources
        .into_iter()
        .map(|s| s.id)
        .collect();
    if let Some(selected) = &r.source_instance_id {
        if !sources.contains(selected) {
            return Err(crate::dto::operation_error(
                "SOURCE_NOT_AUTHORIZED",
                "Collection source is outside the current roots",
            ));
        }
        return Ok(vec![selected.clone()]);
    }
    Ok(sources)
}
pub fn dispatch(r: Request) -> Result<Response> {
    dispatch_inner(
        &directory()?,
        r,
        Some(&crate::storage::data_home()?.join("live-v2/index.sqlite")),
    )
}
#[cfg(test)]
fn dispatch_at(root: &Path, r: Request) -> Result<Response> {
    dispatch_inner(root, r, None)
}
fn dispatch_inner(root: &Path, r: Request, index: Option<&Path>) -> Result<Response> {
    if r.mode.is_some() != (r.action == Action::Configure)
        || r.limit.is_some_and(|n| n == 0 || n > 200)
        || r.after
            .is_some_and(|n| n > crate::adapters::contract::MAX_SAFE_INTEGER)
        || r.action != Action::Events && (r.after.is_some() || r.limit.is_some())
        || r.project.as_ref().is_some_and(|p| {
            p.len() > 4096 || !Path::new(p).is_absolute() || p.chars().any(char::is_control)
        })
    {
        return Err(crate::dto::operation_error(
            "INVALID_ARGUMENT",
            "Invalid collection request",
        ));
    }
    let sources = scope(&r)?;
    let mut db = open(root)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    match r.action {
        Action::Configure => {
            tx.execute(
                "UPDATE settings SET mode=?1 WHERE id=1",
                [if r.mode == Some(Mode::Hooks) {
                    "hooks"
                } else {
                    "logs"
                }],
            )?;
        }
        Action::Pause => {
            tx.execute("UPDATE settings SET paused=1 WHERE id=1", [])?;
        }
        Action::Resume => {
            tx.execute("UPDATE events SET buffered=0", [])?;
            tx.execute("UPDATE settings SET paused=0 WHERE id=1", [])?;
        }
        Action::Status | Action::Events => {}
    }
    let (mode, paused): (String, bool) =
        tx.query_row("SELECT mode,paused FROM settings WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    let mode = if mode == "hooks" {
        Mode::Hooks
    } else {
        Mode::Logs
    };
    let source_json = serde_json::to_string(&sources)?;
    let filter = "source IN (SELECT value FROM json_each(?1)) AND (?2 IS NULL OR project=?2)";
    let (received,buffered,last_received_at,identity_unknown):(u64,u64,Option<String>,u64)=tx.query_row(&format!("SELECT COUNT(*),COALESCE(SUM(buffered),0),MAX(received_at),COALESCE(SUM(NOT json_extract(payload,'$.nativeIdentity')),0) FROM events WHERE {filter}"),params![source_json,r.project],|row|Ok((count(row,0)?,count(row,1)?,row.get(2)?,count(row,3)?)))?;
    let gaps: u64 = tx.query_row(
        &format!("SELECT MIN(COALESCE(SUM(count),0),9007199254740991) FROM gaps WHERE {filter}"),
        params![source_json, r.project],
        |row| count(row, 0),
    )?;
    let mut events = vec![];
    if r.action == Action::Events {
        let mut statement = tx.prepare(&format!("SELECT sequence,received_at,buffered,payload FROM events WHERE {filter} AND sequence>?3 ORDER BY sequence LIMIT ?4"))?;
        let rows = statement.query_map(
            params![
                source_json,
                r.project,
                i64::try_from(r.after.unwrap_or(0))?,
                r.limit.unwrap_or(50) + 1
            ],
            |row| {
                Ok((
                    count(row, 0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )?;
        for row in rows {
            let (sequence, received_at, buffered, payload) = row?;
            events.push(EventRow {
                sequence,
                received_at,
                buffered,
                observation: serde_json::from_str(&payload)?,
                association: association::initial(),
            });
        }
    }
    let next_after = if events.len() > r.limit.unwrap_or(50) as usize {
        events.pop();
        events.last().map(|e| e.sequence)
    } else {
        None
    };
    if let Some(file) = index {
        association::associate(&mut events, file);
    }
    let state = if paused {
        State::Paused
    } else if mode == Mode::Logs {
        State::LogsOnly
    } else if received > 0 {
        State::Received
    } else {
        State::Waiting
    };
    tx.commit()?;
    Ok(Response {
        output_version: 1,
        action: r.action,
        checked_at: chrono::Utc::now().to_rfc3339(),
        mode,
        state,
        received,
        buffered,
        gaps,
        identity_unknown,
        last_received_at,
        events,
        next_after,
        event_limit: EVENT_LIMIT,
        buffer_limit: BUFFER_LIMIT,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> String {
        crate::adapters::stable_id(&[
            "codex",
            dunce::canonicalize(std::env::temp_dir())
                .unwrap()
                .to_str()
                .unwrap(),
        ])
    }
    fn request() -> Request {
        Request {
            roots: Some(vec![
                dunce::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ]),
            ..Request::default()
        }
    }

    fn enable(dir: &Path) {
        dispatch_at(
            dir,
            Request {
                action: Action::Configure,
                mode: Some(Mode::Hooks),
                ..request()
            },
        )
        .unwrap();
    }
    fn fixture() -> Observation {
        safe_observation(&serde_json::json!({"hook_event_name":"PostToolUse","session_id":"native-session","turn_id":"turn","tool_use_id":"call","cwd":std::env::temp_dir(),"tool_input":{"secret":"PRIVATE_INPUT"},"tool_response":"PRIVATE_OUTPUT","prompt":"PRIVATE_PROMPT"}),&source()).unwrap()
    }
    #[test]
    fn private_events_replay_without_duplicates_or_bodies() {
        let d = tempfile::tempdir().unwrap();
        enable(d.path());
        let o = fixture();
        for _ in 0..3 {
            ingest_at(d.path(), o.clone(), 20, 2).unwrap();
        }
        let r = dispatch_at(
            d.path(),
            Request {
                action: Action::Events,
                ..request()
            },
        )
        .unwrap();
        assert_eq!(r.received, 1);
        assert_eq!(r.identity_unknown, 0);
        assert_eq!(r.events.len(), 1);
        assert!(r.events[0].observation.occurred_at.is_none());
        let bytes = std::fs::read(d.path().join("observations.sqlite")).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for private in ["PRIVATE_INPUT", "PRIVATE_OUTPUT", "PRIVATE_PROMPT"] {
            assert!(!text.contains(private));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(d.path().join("observations.sqlite"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn paused_buffer_overflow_and_restart_retain_gaps() {
        let d = tempfile::tempdir().unwrap();
        enable(d.path());
        dispatch_at(
            d.path(),
            Request {
                action: Action::Pause,
                ..request()
            },
        )
        .unwrap();
        for n in 0..4 {
            let mut o = fixture();
            o.id = format!("event-{n}");
            ingest_at(d.path(), o, 20, 2).unwrap();
        }
        let r = dispatch_at(d.path(), request()).unwrap();
        assert_eq!(r.received, 2);
        assert_eq!(r.buffered, 2);
        assert_eq!(r.gaps, 2);
        let r = dispatch_at(
            d.path(),
            Request {
                action: Action::Resume,
                ..request()
            },
        )
        .unwrap();
        assert_eq!(r.buffered, 0);
        assert_eq!(r.received, 2);
        assert_eq!(r.gaps, 2);
    }
    #[test]
    fn missing_identity_is_never_merged_by_content() {
        let d = tempfile::tempdir().unwrap();
        enable(d.path());
        let body = serde_json::json!({"hook_event_name":"Stop","session_id":"native-session"});
        for _ in 0..2 {
            ingest_at(d.path(), safe_observation(&body, &source()).unwrap(), 20, 2).unwrap();
        }
        let r = dispatch_at(d.path(), request()).unwrap();
        assert_eq!(r.received, 2);
        assert_eq!(r.identity_unknown, 2);
    }
    #[test]
    fn scope_pagination_conflicts_limits_and_unknown_formats() {
        let d = tempfile::tempdir().unwrap();
        enable(d.path());
        let first = fixture();
        ingest_at(d.path(), first.clone(), 2, 2).unwrap();
        let mut conflicting = first.clone();
        conflicting.turn_id = Some("conflict".into());
        ingest_at(d.path(), conflicting, 2, 2).unwrap();
        for n in 1..3 {
            let mut o = first.clone();
            o.id = format!("id-{n}");
            ingest_at(d.path(), o, 2, 2).unwrap();
        }
        let r = dispatch_at(
            d.path(),
            Request {
                action: Action::Events,
                limit: Some(1),
                ..request()
            },
        )
        .unwrap();
        assert_eq!(r.received, 2);
        assert_eq!(r.gaps, 2);
        assert!(r.next_after.is_some());
        let next = dispatch_at(
            d.path(),
            Request {
                action: Action::Events,
                after: r.next_after,
                limit: Some(1),
                ..request()
            },
        )
        .unwrap();
        assert_eq!(next.events.len(), 1);
        assert!(next.next_after.is_none());
        let outside = dispatch_at(
            d.path(),
            Request {
                project: Some(
                    std::env::temp_dir()
                        .join("outside")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..request()
            },
        )
        .unwrap();
        assert_eq!(outside.received, 0);
        assert_eq!(outside.gaps, 0);
        let selected = dispatch_at(
            d.path(),
            Request {
                source_instance_id: Some(source()),
                ..request()
            },
        )
        .unwrap();
        assert_eq!(selected.received, 2);
        assert!(
            dispatch_at(
                d.path(),
                Request {
                    source_instance_id: Some("foreign-source".into()),
                    ..request()
                }
            )
            .is_err()
        );
        let db = open(d.path()).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
        drop(db);
        assert!(open(d.path()).is_err());
    }
    #[test]
    fn default_mode_does_not_collect_and_invalid_input_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        ingest_at(d.path(), fixture(), 2, 2).unwrap();
        assert_eq!(dispatch_at(d.path(), request()).unwrap().received, 0);
        assert!(
            safe_observation(
                &serde_json::json!({"hook_event_name":"Unknown","session_id":"id"}),
                "s"
            )
            .is_err()
        );
        assert!(
            safe_observation(
                &serde_json::json!({"hook_event_name":"Stop","session_id":"secret with spaces"}),
                "s"
            )
            .is_err()
        );
    }
}
