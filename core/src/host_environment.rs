//! A task scheduler cannot supply per-task environment variables on every host.
//! Load a bounded, installation-owned startup configuration before threads start.
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, ffi::OsString, path::Path, sync::OnceLock};
static VALUES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    version: u32,
    values: BTreeMap<String, String>,
}
pub fn configure(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        path.is_absolute()
            && metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() <= 32768,
        "Invalid service startup configuration"
    );
    let configuration: Configuration = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        configuration.version == 1,
        "Unsupported service startup configuration"
    );
    for (key, value) in &configuration.values {
        ensure!(
            matches!(
                key.as_str(),
                "CODEX_HOME"
                    | "WOMBAT_DATA_HOME"
                    | "WOMBAT_ENDPOINT_FILE"
                    | "XDG_DATA_HOME"
                    | "LOCALAPPDATA"
            ),
            "Unsupported service environment key"
        );
        ensure!(
            Path::new(value).is_absolute() && !value.contains('\0'),
            "Service paths must be absolute"
        );
    }
    VALUES
        .set(configuration.values)
        .map_err(|_| anyhow::anyhow!("Service startup configuration already loaded"))
}
pub(crate) fn get(key: &str) -> Option<OsString> {
    if let Some(values) = VALUES.get() {
        values.get(key).map(OsString::from)
    } else {
        std::env::var_os(key)
    }
}
pub(crate) fn context() -> Result<serde_json::Value> {
    let mut values = serde_json::Map::new();
    for key in [
        "CODEX_HOME",
        "WOMBAT_DATA_HOME",
        "XDG_DATA_HOME",
        "LOCALAPPDATA",
    ] {
        let value = get(key)
            .map(crate::absolute)
            .transpose()?
            .map(|p| p.to_string_lossy().into_owned());
        values.insert(key.into(), serde_json::to_value(value)?);
    }
    values.insert(
        "HOME".into(),
        serde_json::to_value(crate::home().to_string_lossy())?,
    );
    Ok(serde_json::Value::Object(values))
}
