//! Explicit, durable directory grants. Historical cwd never creates a grant.
use anyhow::Result;
use rusqlite::{Connection, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
/// Canonical directory identity does not grant access; callers retain explicit roots/grants.
pub(crate) fn canonical_project(path: &str) -> Result<String> {
    let canonical = dunce::canonicalize(path).map_err(|_| {
        crate::dto::operation_error("INVALID_ARGUMENT", "Project directory is inaccessible")
    })?;
    if !canonical.is_dir() {
        return Err(crate::dto::operation_error(
            "INVALID_ARGUMENT",
            "Project root must be a directory",
        ));
    }
    Ok(canonical.to_string_lossy().into_owned())
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    List,
    Authorize,
    Revoke,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Source,
    Project,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub action: Action,
    pub purpose: Option<Purpose>,
    pub path: Option<String>,
    pub grant_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub id: String,
    pub purpose: Purpose,
    pub path: String,
    pub directory_identity: String,
    pub authorized_at: String,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub output_version: u32,
    pub action: Action,
    pub grants: Vec<Grant>,
}
fn db_path() -> Result<PathBuf> {
    Ok(crate::storage::data_home()?.join("user-v1/directories.sqlite3"))
}
fn identity(path: &str) -> Result<String> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(crate::dto::operation_error("INVALID_ARGUMENT", "需要目录"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(crate::hash(format!(
            "{}:{}",
            dunce::canonicalize(path)?.to_string_lossy(),
            meta.creation_time()
        )))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(crate::hash(
            dunce::canonicalize(path)?.to_string_lossy().as_bytes(),
        ))
    }
}
fn connection() -> Result<Connection> {
    let path = db_path()?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path.parent().unwrap())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(crate::dto::operation_error(
            "AUTHORIZATION_UNAVAILABLE",
            "授权记录无法读取",
        ));
    }
    let db = Connection::open(path)?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY,purpose TEXT NOT NULL,path TEXT NOT NULL,payload BLOB NOT NULL,UNIQUE(purpose,path));")?;
    Ok(db)
}
pub(crate) fn grants() -> Result<Vec<Grant>> {
    if !db_path()?.exists() {
        return Ok(vec![]);
    }
    let db = connection()?;
    let mut stmt = db.prepare("SELECT json(payload) FROM grants ORDER BY path,purpose")?;
    let raw = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raw.into_iter()
        .map(|text| {
            let mut grant: Grant = serde_json::from_str(&text)?;
            grant.status = if dunce::canonicalize(&grant.path).ok().as_ref()
                == Some(&PathBuf::from(&grant.path))
                && identity(&grant.path).ok().as_ref() == Some(&grant.directory_identity)
                && fs::read_dir(&grant.path).is_ok()
            {
                "authorized"
            } else {
                "unavailable"
            }
            .into();
            Ok(grant)
        })
        .collect()
}
pub(crate) fn authorized(purpose: Purpose) -> Result<Vec<String>> {
    Ok(grants()?
        .into_iter()
        .filter(|g| g.purpose == purpose && g.status == "authorized")
        .map(|g| g.path)
        .collect())
}
pub fn dispatch(r: Request) -> Result<Response> {
    match r.action {
        Action::Authorize => {
            let path =
                dunce::canonicalize(r.path.as_ref().ok_or_else(|| {
                    crate::dto::operation_error("INVALID_ARGUMENT", "需要明确目录")
                })?)?
                .to_string_lossy()
                .into_owned();
            let purpose = r
                .purpose
                .ok_or_else(|| crate::dto::operation_error("INVALID_ARGUMENT", "需要授权用途"))?;
            let id = identity(&path)?;
            fs::read_dir(&path)?;
            let mut db = connection()?;
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM grants", [], |row| row.get(0))?;
            let existing: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM grants WHERE purpose=?1 AND path=?2)",
                params![serde_json::to_string(&purpose)?, path],
                |row| row.get(0),
            )?;
            if count >= 64 && !existing {
                return Err(crate::dto::operation_error(
                    "RESOURCE_LIMIT",
                    "授权目录达到上限",
                ));
            }
            let grant = Grant {
                id: uuid::Uuid::new_v4().to_string(),
                purpose: purpose.clone(),
                path: path.clone(),
                directory_identity: id,
                authorized_at: chrono::Utc::now().to_rfc3339(),
                status: "authorized".into(),
            };
            tx.execute("INSERT INTO grants(id,purpose,path,payload)VALUES(?1,?2,?3,jsonb(?4))ON CONFLICT(purpose,path)DO UPDATE SET id=excluded.id,payload=excluded.payload",params![grant.id,serde_json::to_string(&purpose)?,path,serde_json::to_string(&grant)?])?;
            tx.commit()?;
        }
        Action::Revoke => {
            let id = r
                .grant_id
                .ok_or_else(|| crate::dto::operation_error("INVALID_ARGUMENT", "需要授权身份"))?;
            connection()?.execute("DELETE FROM grants WHERE id=?1", [id])?;
        }
        Action::List => {
            if r.path.is_some() {
                return Err(crate::dto::operation_error(
                    "INVALID_ARGUMENT",
                    "读取不能授权目录",
                ));
            }
        }
    }
    Ok(Response {
        output_version: 1,
        action: r.action,
        grants: grants()?,
    })
}
