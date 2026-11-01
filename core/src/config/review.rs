//! Ephemeral native catalog facts; the ledger and source-use reducer remain unchanged.
use super::*;
use serde::Deserialize;

pub(super) const RENDERER: &str = "codex-unaliased-reference-row-v1";
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Capture {
    pub native_version: Option<String>,
    pub checked_at: Option<String>,
    pub error_code: Option<String>,
    pub contexts: Vec<CapturedContext>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CapturedContext {
    pub cwd: String,
    pub complete: bool,
    pub skills: Vec<CapturedSkill>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CapturedSkill {
    pub name: String,
    pub description: String,
    pub path: String,
    pub enabled: bool,
    pub scope: String,
}
pub(super) fn valid(capture: &Capture) -> bool {
    capture
        .native_version
        .as_ref()
        .is_some_and(|v| !v.is_empty() && v.len() <= 256)
        && capture
            .checked_at
            .as_deref()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .is_some_and(|at| (Utc::now() - at.with_timezone(&Utc)).num_seconds().abs() <= 60)
        && capture.contexts.len() <= 64
        && capture
            .contexts
            .iter()
            .map(|c| c.skills.len())
            .sum::<usize>()
            <= 4096
}
pub(super) fn bind(items: &mut [Item], capture: Option<&Capture>, projects: &[String]) {
    let capture = capture.filter(|c| valid(c));
    for item in items
        .iter_mut()
        .filter(|i| matches!(i.kind, Kind::Skill | Kind::Mcp | Kind::Hook))
    {
        let mut footprint = ContextFootprint {
            status: "unmeasured".into(),
            estimate: None,
            renderer: None,
            native_version: capture.and_then(|c| c.native_version.clone()),
            checked_at: capture.and_then(|c| c.checked_at.clone()),
            contexts: vec![],
            visibility: "unconfirmed".into(),
        };
        if item.kind == Kind::Skill && item.current && !item.stale {
            let matches: Vec<_> = capture
                .into_iter()
                .flat_map(|c| &c.contexts)
                .filter(|c| projects.contains(&c.cwd))
                .flat_map(|c| {
                    c.skills
                        .iter()
                        .filter(|s| s.path == item.path)
                        .map(move |s| (c, s))
                })
                .collect();
            // A physical entry cannot receive two conflicting estimates or enablement states
            // in the same project. Preserve unknown instead of selecting the first record.
            let unique = matches.iter().all(|(c, s)| {
                matches.iter().all(|(other, t)| {
                    (c.cwd != other.cwd || s.enabled == t.enabled)
                        && s.name == t.name
                        && s.description == t.description
                })
            });
            if unique && let Some((_, skill)) = matches.first() {
                let row = format!(
                    "- {}: {} (file: {})\n",
                    skill.name, skill.description, skill.path
                );
                footprint.estimate = measure::estimate(&row, &crate::hash(&row)).map(|mut e| {
                    e.payload = "skillDiscoveryMetadata".into();
                    e
                });
                footprint.status = if footprint.estimate.is_some() {
                    "estimated"
                } else {
                    "unmeasured"
                }
                .into();
                footprint.renderer = Some(RENDERER.into());
                for (context, skill) in matches {
                    if !footprint.contexts.iter().any(|c| c.project == context.cwd) {
                        footprint.contexts.push(SkillContext {
                            project: context.cwd.clone(),
                            name: skill.name.clone(),
                            enabled: skill.enabled,
                            scope: skill.scope.clone(),
                        });
                    }
                }
                footprint.contexts.sort_by(|a, b| a.project.cmp(&b.project));
            }
        }
        item.context_footprint = Some(footprint);
    }
}
pub(super) fn tokens(item: &Item) -> Option<u64> {
    item.context_footprint
        .as_ref()?
        .estimate
        .as_ref()
        .map(|e| e.tokens)
}
pub(super) fn state(item: &Item) -> u8 {
    let Some(f) = &item.context_footprint else {
        return 2;
    };
    if f.contexts.iter().any(|c| c.enabled) {
        0
    } else if !f.contexts.is_empty() {
        1
    } else {
        2
    }
}
pub(super) fn compare(a: &Item, b: &Item) -> std::cmp::Ordering {
    // Compare only like kinds and counting units. Unknown follows measured values.
    fn kind(item: &Item) -> u8 {
        match item.kind {
            Kind::Skill => 0,
            Kind::Mcp => 1,
            Kind::Hook => 2,
            Kind::Rule => 3,
        }
    }
    fn unit(item: &Item) -> u8 {
        match item.use_basis.as_ref().map(|b| &b.unit) {
            Some(UseUnit::ObjectUse) => 0,
            Some(UseUnit::RuleRead) => 1,
            Some(UseUnit::RuleLoadOrRead) => 2,
            None => 3,
        }
    }
    fn completeness(item: &Item) -> u8 {
        match item.use_basis.as_ref().map(|b| &b.status) {
            Some(UseBasisStatus::Observed) => 0,
            Some(UseBasisStatus::Partial) => 1,
            _ => 2,
        }
    }
    kind(a)
        .cmp(&kind(b))
        .then(state(a).cmp(&state(b)))
        .then(unit(a).cmp(&unit(b)))
        .then(completeness(a).cmp(&completeness(b)))
        .then(a.usage_count.is_none().cmp(&b.usage_count.is_none()))
        .then(a.usage_count.cmp(&b.usage_count))
        .then(tokens(b).cmp(&tokens(a)))
}
pub(super) fn summary(items: &[Item]) -> ExtensionReview {
    let selected: Vec<_> = items
        .iter()
        .filter(|i| matches!(i.kind, Kind::Skill | Kind::Mcp | Kind::Hook))
        .collect();
    ExtensionReview {
        method_version: 1,
        enabled_items: selected.iter().filter(|i| state(i) == 0).count(),
        disabled_items: selected.iter().filter(|i| state(i) == 1).count(),
        unconfirmed_items: selected.iter().filter(|i| state(i) == 2).count(),
        estimated_items: selected.iter().filter(|i| tokens(i).is_some()).count(),
        unmeasured_items: selected.iter().filter(|i| tokens(i).is_none()).count(),
        confirmed_visible_tokens: None,
    }
}
