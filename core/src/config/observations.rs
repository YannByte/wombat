//! Independent, body-free configuration/analysis/host versions for one fixed view.
use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub(crate) const FORMAT_VERSION: u32 = 1;
/// Collector-owned facts, captured before analysis adds its own issues.
#[derive(Default)]
pub(crate) struct ConfigCollection {
    pub issues: Vec<Issue>,
    /// Effective discovery roots include persistent source authorization.
    pub source_roots: Vec<String>,
}
impl ConfigCollection {
    pub(super) fn capture(issues: &[Issue], source_roots: Vec<String>) -> Self {
        Self {
            issues: issues
                .iter()
                .filter(|issue| {
                    !matches!(
                        issue.code.as_str(),
                        "effectiveConfigUnknown" | "hookEffectiveRegistryUnavailable"
                    )
                })
                .cloned()
                .collect(),
            source_roots,
        }
    }
    fn fact<'a>(&'a self, items: &[Item]) -> impl Serialize + 'a {
        let mut issues: Vec<_> = self
            .issues
            .iter()
            .map(|issue| (&issue.code, &issue.path))
            .collect();
        issues.sort_unstable();
        issues.dedup();
        let resource_limited = self.issues.iter().any(|i| i.code == "resourceLimited");
        let missing = items
            .iter()
            .any(|item| item.measurement_status == "missing");
        let failed = self.issues.iter().any(|i| i.code != "resourceLimited")
            || items
                .iter()
                .any(|item| !matches!(item.measurement_status.as_str(), "complete" | "missing"));
        (
            // Keep all four states so resource limits cannot mask a concurrent failure.
            (
                !resource_limited && !missing && !failed,
                missing,
                failed,
                resource_limited,
            ),
            issues,
            members(&self.source_roots),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ObservationVersions {
    pub format_version: u32,
    pub config_revision: String,
    pub analysis_revision: String,
    pub host_revision: String,
    /// Evaluation time is independent of the content revisions and host observation time.
    pub cutoff: String,
}
impl ObservationVersions {
    pub(super) fn combined_revision(&self) -> Result<String> {
        hash_safe(&(
            self.format_version,
            &self.config_revision,
            &self.analysis_revision,
            &self.host_revision,
        ))
    }
}
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Stream safe typed facts into the maintained hash implementation, without a second encoding.
pub(super) fn hash_safe(value: &impl Serialize) -> Result<String> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, value)?;
    Ok(format!("{:x}", writer.0.finalize()))
}
fn members(values: &[String]) -> Vec<&str> {
    let mut values: Vec<_> = values.iter().map(String::as_str).collect();
    values.sort_unstable();
    values.dedup();
    values
}
fn config_fact(item: &Item) -> impl Serialize + '_ {
    let mut contexts: Vec<_> = item
        .source_contexts
        .iter()
        .map(|context| {
            (
                &context.inventory_id,
                &context.source_instance_id,
                context.global,
                &context.content_hash,
                &context.configured_state,
            )
        })
        .collect();
    contexts.sort_unstable();
    // Historical counters and observation timestamps are not current-content measurements.
    (
        (
            &item.id,
            &item.kind,
            &item.name,
            &item.path,
            &item.native_key,
            &item.project,
            &item.source_instance_id,
        ),
        (members(&item.authorized_projects), contexts),
        (
            &item.content_hash,
            &item.configured_state,
            item.current,
            item.stale,
        ),
        (
            item.bytes,
            item.characters,
            &item.bytes_source,
            &item.measurement_status,
        ),
        (item.content_tokens, &item.estimate_status, &item.estimate),
        (
            &item.skill_metadata,
            &item.body_token_estimate,
            &item.body_estimate_status,
        ),
    )
}
pub(super) fn derive(view: &View) -> Result<ObservationVersions> {
    let cutoff = DateTime::parse_from_rfc3339(&view.checked)
        .map_err(|_| operation_error("INVALID_FACTS", "配置观察截止时间无效"))?
        .to_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let mut items: Vec<_> = view.items.iter().collect();
    items.sort_unstable_by(|a, b| {
        (&a.id, &a.path, &a.source_instance_id, &a.content_hash).cmp(&(
            &b.id,
            &b.path,
            &b.source_instance_id,
            &b.content_hash,
        ))
    });
    let config_revision = hash_safe(&(
        FORMAT_VERSION,
        "configuration",
        members(&view.roots),
        members(&view.project_roots),
        members(&view.projects),
        view.config_collection.fact(&view.items),
        items.into_iter().map(config_fact).collect::<Vec<_>>(),
    ))?;
    let analysis_revision = view.analysis.observation_revision()?;
    // A repeated fresh capture of identical registrations keeps its content revision.
    // Its actual observation time remains in HookRegistry.checked_at, separate from cutoff.
    let mut contexts = Vec::with_capacity(view.hook_registry.contexts.len());
    for context in &view.hook_registry.contexts {
        let mut registrations = context
            .registrations
            .iter()
            .map(hash_safe)
            .collect::<Result<Vec<_>>>()?;
        registrations.sort_unstable();
        contexts.push((&context.project, context.complete, registrations));
    }
    contexts.sort_unstable();
    let host_revision = hash_safe(&(
        FORMAT_VERSION,
        "host",
        &view.hook_registry.native_version,
        &view.hook_registry.status,
        members(&view.roots),
        members(&view.config_collection.source_roots),
        members(&view.project_roots),
        members(&view.projects),
        contexts,
        view.items
            .iter()
            .filter_map(|item| {
                item.context_footprint.as_ref().map(|f| {
                    (
                        &item.id,
                        &f.status,
                        &f.estimate,
                        &f.renderer,
                        &f.native_version,
                        &f.contexts,
                        &f.visibility,
                    )
                })
            })
            .collect::<Vec<_>>(),
    ))?;
    Ok(ObservationVersions {
        format_version: FORMAT_VERSION,
        config_revision,
        analysis_revision,
        host_revision,
        cutoff,
    })
}
impl View {
    /// Published views initialize this before insertion. A fallible lazy initialization
    /// also supports synthetic views; failed derivations never enter the cache.
    pub(crate) fn observation_versions(&self) -> Result<&ObservationVersions> {
        if let Some(versions) = self.observation_versions.get() {
            return Ok(versions);
        }
        let versions = derive(self)?;
        Ok(self.observation_versions.get_or_init(|| versions))
    }
}

#[cfg(test)]
mod tests;
