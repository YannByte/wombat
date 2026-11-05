//! Read only selected native Skill files, never recursively traverse plugin caches.
use super::*;
use crate::config::review;
pub(super) fn append(
    out: &mut Inventory,
    capture: Option<&review::Capture>,
    sources: &[SourceInstance],
    projects: &[String],
    now: &str,
) {
    let Some(capture) = capture.filter(|c| review::valid(c)) else {
        return;
    };
    let home = crate::host_environment::get("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::home().join(".codex"));
    let Ok(home) = dunce::canonicalize(home) else {
        return;
    };
    let Some(source) = sources
        .iter()
        .find(|s| dunce::canonicalize(&s.root).ok().as_ref() == Some(&home))
    else {
        return;
    };
    let user_skills = dunce::canonicalize(crate::home().join(".agents/skills")).ok();
    let mut seen = BTreeSet::new();
    for context in &capture.contexts {
        if !projects.contains(&context.cwd) {
            continue;
        }
        if !context.complete {
            out.issue("nativeSkillListPartial", Path::new(&context.cwd));
        }
        for skill in &context.skills {
            let path = Path::new(&skill.path);
            // Visibility in a project does not make a global managed file project-owned.
            let owner = projects
                .iter()
                .filter(|project| path.starts_with(project))
                .max_by_key(|project| project.len())
                .map(String::as_str);
            if path.file_name().and_then(|s| s.to_str()) != Some("SKILL.md")
                || !seen.insert(path.to_owned())
            {
                continue;
            }
            if let Some(item) = out
                .items
                .iter_mut()
                .find(|i| i.kind == Kind::Skill && i.path == skill.path)
            {
                for c in &capture.contexts {
                    if projects.contains(&c.cwd)
                        && c.skills.iter().any(|s| s.path == skill.path)
                        && !item.authorized_projects.contains(&c.cwd)
                    {
                        item.authorized_projects.push(c.cwd.clone());
                    }
                }
                continue;
            }
            let allowed = std::iter::once(home.as_path())
                .chain(projects.iter().map(Path::new))
                .chain(user_skills.as_deref())
                .find(|root| path.starts_with(root));
            let Some(allowed) = allowed else {
                // Listing metadata is authorized by the selected native home/project.
                // Do not drop symlinked external Skills or widen filesystem read roots.
                let mut row = item(
                    path,
                    source,
                    owner,
                    Kind::Skill,
                    skill.name.clone(),
                    None,
                    "nativeListed",
                    "",
                    now,
                    Some(""),
                );
                row.bytes = None;
                row.bytes_source = None;
                row.measurement_status = "nativeMetadataOnly".into();
                row.estimate_status = "contentUnavailable".into();
                row.authorized_projects = capture
                    .contexts
                    .iter()
                    .filter(|c| {
                        projects.contains(&c.cwd) && c.skills.iter().any(|s| s.path == skill.path)
                    })
                    .map(|c| c.cwd.clone())
                    .collect();
                if out.items.len() < ENTRY_LIMIT {
                    out.items.push(row);
                } else {
                    out.issue("resourceLimited", path);
                }
                continue;
            };
            if !dunce::canonicalize(path).is_ok_and(|p| p == path) {
                out.issue("configUnreadable", path);
                continue;
            }
            let first = out.items.len();
            out.text_item(path, allowed, source, owner, Kind::Skill, now);
            for item in &mut out.items[first..] {
                item.authorized_projects = capture
                    .contexts
                    .iter()
                    .filter(|c| {
                        projects.contains(&c.cwd) && c.skills.iter().any(|s| s.path == skill.path)
                    })
                    .map(|c| c.cwd.clone())
                    .collect();
            }
        }
    }
    // Native-only additions share the same physical identity as ordinary discovery.
    crate::config::identity::aggregate(&mut out.items, &mut out.issues);
}
