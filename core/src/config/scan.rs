//! Bounded, allowlisted filesystem discovery. Configuration commands are never executed.
use super::super::config_dto::*;
use crate::adapters::contract::SourceInstance;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const FILE_LIMIT: u64 = 10 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 64 * 1024 * 1024;
const ENTRY_LIMIT: usize = 10_000;
// Case-insensitive filesystems still need an exact product entry name.
fn exact_entry(path: &Path) -> std::io::Result<bool> {
    let entries = fs::read_dir(
        path.parent()
            .ok_or_else(|| std::io::Error::other("parent"))?,
    )?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("name"))?;
    for (n, entry) in entries.enumerate() {
        if n >= ENTRY_LIMIT {
            return Err(std::io::Error::other("limit"));
        }
        let entry = entry?;
        if entry.file_name() == name {
            return Ok(true);
        }
    }
    Ok(false)
}
#[derive(Default)]
pub(super) struct Inventory {
    pub items: Vec<Item>,
    pub issues: Vec<Issue>,
    used: u64,
    visited: BTreeSet<PathBuf>,
    examined: usize,
    estimated: usize,
    pub authorized_roots: BTreeSet<String>,
    pub read_paths: BTreeSet<String>,
}
mod extensions;
mod hook_measurement;
mod hooks_json;
mod instructions;
mod native_hooks;
mod native_skills;
mod reading;
#[cfg(test)]
mod tests;
#[allow(clippy::too_many_arguments)]
fn item(
    path: &Path,
    source: &SourceInstance,
    project: Option<&str>,
    kind: Kind,
    name: String,
    key: Option<String>,
    state: &str,
    text: &str,
    now: &str,
    content_hash: Option<&str>,
) -> Item {
    let path = path.to_string_lossy().into_owned();
    Item {
        id: crate::hash(format!(
            "{}:{path}:{kind:?}:{}",
            source.id,
            key.as_deref().unwrap_or("")
        )),
        name,
        kind: kind.clone(),
        source_instance_id: source.id.clone(),
        path,
        project: project.map(str::to_owned),
        authorized_projects: project.into_iter().map(str::to_owned).collect(),
        source_contexts: vec![],
        native_key: key,
        configured_state: state.into(),
        content_hash: content_hash.map_or_else(|| crate::hash(text), str::to_owned),
        observed_at: now.into(),
        current: true,
        stale: false,
        bytes: Some(text.len() as u64),
        content_tokens: None,
        estimate_status: "tokenizerUnavailable".into(),
        characters: None,
        measurement_status: "complete".into(),
        bytes_source: Some("utf8Payload".into()),
        estimate: None,
        skill_metadata: None,
        body_token_estimate: None,
        body_estimate_status: "unknown".into(),
        context_footprint: None,
        usage_count: None,
        use_basis: None,
        last_record_at: None,
        observation: Observation::Unknown,
        counts: Counts::default(),
        related_turns: 0,
        related_tasks: 0,
        usage: None,
    }
}
fn source_context(row: &Item, source_id: String) -> SourceContext {
    SourceContext {
        inventory_id: crate::hash(format!(
            "{}:{}:{:?}:{}",
            source_id,
            row.path,
            row.kind,
            row.native_key.as_deref().unwrap_or("")
        )),
        source_instance_id: source_id,
        content_hash: row.content_hash.clone(),
        configured_state: row.configured_state.clone(),
        global: row.project.is_none(),
        counts: Counts::default(),
        observation: Observation::Unknown,
        last_record_at: None,
    }
}
#[cfg(test)]
pub(super) fn scan(sources: &[SourceInstance], projects: &[String], now: &str) -> Inventory {
    scan_selected(sources, projects, now, &[])
}
pub(super) fn scan_selected(
    sources: &[SourceInstance],
    projects: &[String],
    now: &str,
    kinds: &[Kind],
) -> Inventory {
    let selected = |kind: Kind| kinds.is_empty() || kinds.contains(&kind);
    let mut out = Inventory::default();
    for source in sources {
        let root = match dunce::canonicalize(&source.root) {
            Ok(p) => p,
            Err(_) => {
                out.issue("configUnreadable", Path::new(&source.root));
                continue;
            }
        };
        out.authorized_roots
            .insert(root.to_string_lossy().into_owned());
        if selected(Kind::Rule) {
            out.text_item(
                &root.join("AGENTS.md"),
                &root,
                source,
                None,
                Kind::Rule,
                now,
            );
        }
        if selected(Kind::Mcp) || selected(Kind::Hook) {
            out.mcp(&root.join("config.toml"), &root, source, None, now);
        }
        if selected(Kind::Hook) {
            out.hooks(&root.join("hooks.json"), &root, source, None, now);
        }
        out.visited.clear();
        if selected(Kind::Skill) {
            for dir in [root.join("skills"), root.join(".agents/skills")] {
                out.skills(&dir, &root, source, None, now, 0);
            }
        }
        if selected(Kind::Skill)
            && dunce::canonicalize(crate::home().join(".codex"))
                .ok()
                .as_ref()
                == Some(&root)
        {
            let dir = crate::home().join(".agents/skills");
            if let Ok(allowed) = dunce::canonicalize(&dir) {
                out.skills(&dir, &allowed, source, None, now, 0);
            }
        }
    }
    for row in &mut out.items {
        row.source_contexts = vec![source_context(row, row.source_instance_id.clone())];
    }
    if let Some(source) = sources.first() {
        for project in projects {
            let first = out.items.len();
            let root = Path::new(project);
            out.authorized_roots.insert(project.clone());
            out.visited.clear();
            if selected(Kind::Mcp) || selected(Kind::Hook) {
                out.mcp(
                    &root.join(".codex/config.toml"),
                    root,
                    source,
                    Some(project),
                    now,
                );
            }
            if selected(Kind::Hook) {
                out.hooks(
                    &root.join(".codex/hooks.json"),
                    root,
                    source,
                    Some(project),
                    now,
                );
            }
            out.visited.clear();
            if selected(Kind::Skill) {
                out.skills(
                    &root.join(".agents/skills"),
                    root,
                    source,
                    Some(project),
                    now,
                    0,
                );
            }
            for row in &mut out.items[first..] {
                row.source_contexts = sources
                    .iter()
                    .map(|s| source_context(row, s.id.clone()))
                    .collect();
            }
        }
        // Explicit configuration locations precede the broad AGENTS walk, so a
        // large project cannot consume the budget before known Skills are read.
        if selected(Kind::Rule) {
            for project in projects {
                let first = out.items.len();
                let root = Path::new(project);
                out.visited.clear();
                out.agents(root, root, source, project, now, 0);
                for row in &mut out.items[first..] {
                    row.source_contexts = sources
                        .iter()
                        .map(|source| source_context(row, source.id.clone()))
                        .collect();
                }
            }
        }
    }
    out.items.sort_by(|a, b| a.id.cmp(&b.id));
    out.items.dedup_by(|a, b| {
        if a.id != b.id {
            return false;
        }
        b.source_contexts.extend(a.source_contexts.iter().cloned());
        b.authorized_projects
            .extend(a.authorized_projects.iter().cloned());
        b.authorized_projects.sort();
        b.authorized_projects.dedup();
        if a.project.as_ref().is_some_and(|p| {
            b.project.as_ref().is_none_or(|old| {
                Path::new(p).components().count() > Path::new(old).components().count()
            })
        }) {
            b.project.clone_from(&a.project);
        }
        true
    });
    super::identity::aggregate(&mut out.items, &mut out.issues);
    out
}

pub(super) fn append_native_hooks(
    out: &mut Inventory,
    capture: Option<&super::hooks::Capture>,
    sources: &[SourceInstance],
    projects: &[String],
    now: &str,
) {
    native_hooks::append(out, capture, sources, projects, now);
}

pub(super) fn append_native_skills(
    out: &mut Inventory,
    capture: Option<&super::review::Capture>,
    sources: &[SourceInstance],
    projects: &[String],
    now: &str,
) {
    native_skills::append(out, capture, sources, projects, now);
}
