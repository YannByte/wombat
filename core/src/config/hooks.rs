//! Native registry observations are contextual, ephemeral and independent of runtime usage.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Capture {
    pub native_version: Option<String>,
    pub checked_at: Option<String>,
    pub contexts: Vec<CapturedContext>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CapturedContext {
    pub cwd: String,
    pub complete: bool,
    pub hooks: Vec<CapturedHook>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CapturedHook {
    pub key: String,
    pub event_name: String,
    pub source_path: String,
    pub source_hash: String,
    pub current_hash: String,
    pub enabled: bool,
    pub trust_status: HookTrust,
    pub handler_type: HookHandler,
    pub source: String,
    pub plugin_id: Option<String>,
    pub command: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Context {
    pub projects: Vec<String>,
    pub roots: Vec<String>,
    pub native_home_selected: bool,
    pub has_declarations: bool,
}
pub(crate) fn context(r: Request) -> Result<Context> {
    validate(&r)?;
    let projects = super::scope::projects(&r)?;
    let sources = crate::adapters::codex::CodexAdapter
        .discover(&DiscoveryRequest {
            roots: r
                .roots
                .unwrap_or_default()
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        })
        .sources;
    let source_roots: BTreeSet<_> = sources
        .iter()
        .filter_map(|s| dunce::canonicalize(&s.root).ok())
        .collect();
    let home = crate::host_environment::get("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".codex"));
    let native_home_selected = dunce::canonicalize(home)
        .ok()
        .is_some_and(|p| source_roots.contains(&p));
    let has_declarations =
        native_home_selected && preflight::has_declarations(&source_roots, &projects);
    let roots = source_roots
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .chain(projects.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Context {
        projects,
        roots,
        native_home_selected,
        has_declarations,
    })
}
fn event(event: &str) -> Option<(&'static str, &'static str)> {
    Some(match event {
        "preToolUse" => ("pre_tool_use", "PreToolUse"),
        "permissionRequest" => ("permission_request", "PermissionRequest"),
        "postToolUse" => ("post_tool_use", "PostToolUse"),
        "preCompact" => ("pre_compact", "PreCompact"),
        "postCompact" => ("post_compact", "PostCompact"),
        "sessionStart" => ("session_start", "SessionStart"),
        "sessionEnd" => ("session_end", "SessionEnd"),
        "userPromptSubmit" => ("user_prompt_submit", "UserPromptSubmit"),
        "subagentStart" => ("subagent_start", "SubagentStart"),
        "subagentStop" => ("subagent_stop", "SubagentStop"),
        "stop" => ("stop", "Stop"),
        "interrupt" => ("interrupt", "Interrupt"),
        _ => return None,
    })
}
pub(super) use keys::declaration;
pub(super) fn valid(capture: &Capture) -> bool {
    let fresh = capture
        .checked_at
        .as_deref()
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .is_some_and(|at| {
            let age = Utc::now().signed_duration_since(at).num_milliseconds();
            (0..60_000).contains(&age)
        });
    fresh
        && capture.contexts.len() <= 64
        && capture
            .contexts
            .iter()
            .map(|c| c.hooks.len())
            .sum::<usize>()
            <= 512
}
pub(super) fn bind(capture: Option<&Capture>, items: &[Item], projects: &[String]) -> HookRegistry {
    let mut registry = HookRegistry::default();
    let Some(capture) = capture.filter(|c| valid(c)) else {
        return registry;
    };
    registry.native_version.clone_from(&capture.native_version);
    registry.checked_at.clone_from(&capture.checked_at);
    let mut declarations: BTreeMap<(&str, &str), Vec<&Item>> = BTreeMap::new();
    for item in items
        .iter()
        .filter(|i| i.kind == Kind::Hook && i.current && !i.stale)
    {
        if let Some(key) = item.native_key.as_deref() {
            declarations
                .entry((&item.path, key))
                .or_default()
                .push(item);
        }
    }
    let mut seen = BTreeSet::new();
    for context in &capture.contexts {
        if !projects.contains(&context.cwd) || !seen.insert(&context.cwd) {
            continue;
        }
        let mut output = HookContext {
            project: context.cwd.clone(),
            complete: context.complete,
            registrations: vec![],
        };
        let mut keys = BTreeSet::new();
        for hook in &context.hooks {
            let mut matches = Vec::new();
            for key in declaration(hook).into_iter().flatten() {
                if let Some(found) = declarations.get(&(hook.source_path.as_str(), key.as_str())) {
                    matches.extend(found.iter().copied());
                }
            }
            if matches.is_empty() {
                output.complete = false;
                continue;
            }
            if matches.len() != 1
                || matches[0].content_hash != hook.source_hash
                || !keys.insert(&hook.key)
                || hook.current_hash.len() > 256
                || hook.current_hash.is_empty()
            {
                output.complete = false;
                continue;
            }
            output.registrations.push(HookRegistration {
                item_id: matches[0].id.clone(),
                native_key: hook.key.clone(),
                content_hash: hook.source_hash.clone(),
                registration_hash: hook.current_hash.clone(),
                enabled: hook.enabled,
                trust: hook.trust_status.clone(),
                handler: hook.handler_type.clone(),
                source: hook.source.clone(),
                plugin_id: hook.plugin_id.clone(),
            });
        }
        registry.contexts.push(output);
    }
    registry.status = if registry.contexts.is_empty() {
        HookRegistryStatus::Unavailable
    } else if registry.contexts.len() == projects.len()
        && registry.contexts.iter().all(|c| c.complete)
    {
        HookRegistryStatus::Observed
    } else {
        HookRegistryStatus::Partial
    };
    registry
}

mod keys;
mod preflight;
#[cfg(test)]
mod tests;
