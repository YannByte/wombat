//! Only native-listed plugin declarations are read; no recursive plugin cache discovery.
use super::*;
use crate::config::hooks;
use std::collections::BTreeMap;

pub(super) fn append(
    out: &mut Inventory,
    capture: Option<&hooks::Capture>,
    sources: &[SourceInstance],
    projects: &[String],
    now: &str,
) {
    let Some(capture) = capture.filter(|c| hooks::valid(c)) else {
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
    append_selected(out, capture, source, &home, projects, now);
}

fn append_selected(
    out: &mut Inventory,
    capture: &hooks::Capture,
    source: &SourceInstance,
    home: &Path,
    projects: &[String],
    now: &str,
) {
    // File -> declaration pointer -> applicable projects. One bounded read per physical file.
    let mut files: BTreeMap<&str, BTreeMap<String, BTreeSet<&str>>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for context in &capture.contexts {
        if !projects.contains(&context.cwd) || !seen.insert(&context.cwd) {
            continue;
        }
        let mut keys = BTreeMap::new();
        for hook in &context.hooks {
            *keys.entry(&hook.key).or_insert(0) += 1;
        }
        for hook in &context.hooks {
            if hook.source != "plugin" || keys.get(&hook.key) != Some(&1) {
                continue;
            }
            let Some(pointers) = hooks::declaration(hook) else {
                continue;
            };
            for pointer in pointers {
                files
                    .entry(&hook.source_path)
                    .or_default()
                    .entry(pointer)
                    .or_default()
                    .insert(&context.cwd);
            }
        }
    }
    if files.is_empty() {
        return;
    }
    let mut additions = Vec::new();
    let mut existing: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, row) in out
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.current && i.kind == Kind::Hook)
    {
        if let Some((file, _)) = files.get_key_value(row.path.as_str()) {
            existing.entry(file).or_default().push(index);
        }
    }
    for (file, declarations) in files {
        let path = Path::new(file);
        let allowed = if path.starts_with(home) {
            home
        } else if let Some(project) = projects.iter().find(|p| path.starts_with(p)) {
            Path::new(project)
        } else {
            out.issue("outsideAuthorizedRoot", path);
            continue;
        };
        if !dunce::canonicalize(path).is_ok_and(|p| p == path) {
            out.issue("configUnreadable", path);
            continue;
        }
        let memberships: BTreeSet<_> = declarations.values().flatten().copied().collect();
        if let Some(indexes) = existing.get(file) {
            for index in indexes {
                let row = &mut out.items[*index];
                if let Some(members) = row.native_key.as_ref().and_then(|k| declarations.get(k)) {
                    row.authorized_projects
                        .extend(members.iter().map(|p| (*p).to_owned()));
                    row.authorized_projects.sort();
                    row.authorized_projects.dedup();
                }
            }
            continue;
        }
        let remaining = ENTRY_LIMIT.saturating_sub(out.items.len() + additions.len());
        if remaining == 0 {
            out.issue("resourceLimited", path);
            continue;
        }
        let first = out.items.len();
        out.authorized_roots
            .insert(allowed.to_string_lossy().into_owned());
        out.plugin_hooks(
            path,
            allowed,
            source,
            memberships.first().copied(),
            now,
            &declarations.keys().cloned().collect(),
        );
        let mut rows = out.items.split_off(first);
        rows.retain(|row| {
            row.native_key
                .as_ref()
                .is_some_and(|k| declarations.contains_key(k))
        });
        if rows.len() > remaining {
            rows.truncate(remaining);
            out.issue("resourceLimited", path);
        }
        for row in &mut rows {
            row.authorized_projects = declarations[row.native_key.as_ref().unwrap()]
                .iter()
                .map(|p| (*p).to_owned())
                .collect();
            row.project = row.authorized_projects.first().cloned();
            row.source_contexts = vec![source_context(row, source.id.clone())];
        }
        additions.extend(rows);
    }
    crate::config::identity::aggregate(&mut additions, &mut out.issues);
    out.items.extend(additions);
    out.items.sort_by(|a, b| a.id.cmp(&b.id));
}

#[cfg(test)]
mod inline_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn native_plugin_inventory_reads_once_and_keeps_only_registered_project_memberships() {
        let dir = tempfile::tempdir().unwrap();
        let home = dunce::canonicalize(dir.path()).unwrap();
        let path = home.join("package/hooks/声明.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"never-execute-private"},{"type":"command","command":"unlisted"}]}]}}"#;
        fs::write(&path, text).unwrap();
        let source = SourceInstance {
            id: "native-source".into(),
            agent_kind: "codex".into(),
            root: home.to_string_lossy().into_owned(),
        };
        let now = Utc::now().to_rfc3339();
        let hook = json!({"key":"sample@test:hooks/声明.json:session_start:0:0","pluginId":"sample@test","source":"plugin","sourcePath":path,"sourceHash":crate::hash(text),"currentHash":"sha256:synthetic","eventName":"sessionStart","enabled":true,"trustStatus":"trusted","handlerType":"command"});
        let capture: hooks::Capture = serde_json::from_value(json!({"nativeVersion":"0.160.0","checkedAt":now,"contexts":[{"cwd":"/a","complete":true,"hooks":[hook]},{"cwd":"/b","complete":true,"hooks":[hook]},{"cwd":"/outside","complete":true,"hooks":[hook]}]})).unwrap();
        let mut inventory = Inventory::default();
        append_selected(
            &mut inventory,
            &capture,
            &source,
            &home,
            &["/a".into(), "/b".into()],
            &now,
        );
        assert_eq!(inventory.used, text.len() as u64);
        assert_eq!(inventory.items.len(), 1);
        let row = &inventory.items[0];
        assert_eq!(row.authorized_projects, ["/a", "/b"]);
        assert_eq!(row.project.as_deref(), Some("/a"));
        assert!(row.source_contexts.iter().all(|c| !c.global));
        assert!(row.applies(None, Some("/a")));
        assert!(!row.applies(None, Some("/unlisted")));
        assert_eq!(row.content_hash, crate::hash(text));
        assert_eq!(
            row.bytes,
            Some(r#"{"type":"command","command":"never-execute-private"}"#.len() as u64)
        );
        assert!(matches!(
            hooks::bind(
                Some(&capture),
                &inventory.items,
                &["/a".into(), "/b".into()]
            )
            .status,
            HookRegistryStatus::Observed
        ));
        assert!(
            !serde_json::to_string(&inventory.items)
                .unwrap()
                .contains("never-execute-private")
        );
        let id = row.id.clone();
        append_selected(
            &mut inventory,
            &capture,
            &source,
            &home,
            &["/a".into(), "/b".into()],
            &now,
        );
        assert_eq!(inventory.used, text.len() as u64);
        assert_eq!(inventory.items[0].id, id);
    }

    #[test]
    fn additions_share_the_inventory_limit_without_rewriting_existing_identities() {
        let dir = tempfile::tempdir().unwrap();
        let home = dunce::canonicalize(dir.path()).unwrap();
        let source = SourceInstance {
            id: "source".into(),
            agent_kind: "codex".into(),
            root: home.to_string_lossy().into_owned(),
        };
        let now = Utc::now().to_rfc3339();
        let mut inventory = super::super::scan(std::slice::from_ref(&source), &[], &now);
        let base = inventory.items[0].clone();
        inventory.items = (0..ENTRY_LIMIT - 2)
            .map(|n| {
                let mut row = base.clone();
                row.id = format!("existing-{n}");
                row
            })
            .collect();
        let mut registrations = vec![];
        fs::create_dir(home.join("hooks")).unwrap();
        for (name, count) in [("one.json", 1), ("two.json", 2)] {
            let path = home.join("hooks").join(name);
            let text = json!({"hooks":{"SessionStart":[{"hooks":vec![json!({"type":"command","command":"never-run"}); count]}]}}).to_string();
            fs::write(&path, &text).unwrap();
            for n in 0..count {
                registrations.push(json!({"key":format!("sample@test:hooks/{name}:session_start:0:{n}"),"pluginId":"sample@test","source":"plugin","sourcePath":path,"sourceHash":crate::hash(&text),"currentHash":"sha256:synthetic","eventName":"sessionStart","enabled":true,"trustStatus":"trusted","handlerType":"command"}));
            }
        }
        let capture: hooks::Capture = serde_json::from_value(json!({"nativeVersion":"0.160.0","checkedAt":now,"contexts":[{"cwd":"/a","complete":true,"hooks":registrations}]})).unwrap();
        append_selected(
            &mut inventory,
            &capture,
            &source,
            &home,
            &["/a".into()],
            &now,
        );
        assert_eq!(inventory.items.len(), ENTRY_LIMIT);
        assert_eq!(
            inventory
                .items
                .iter()
                .filter(|i| i.id.starts_with("existing-"))
                .count(),
            ENTRY_LIMIT - 2
        );
        assert!(inventory.issues.iter().any(|i| i.code == "resourceLimited"));
        let registry = hooks::bind(Some(&capture), &inventory.items, &["/a".into()]);
        assert!(matches!(registry.status, HookRegistryStatus::Partial));
        assert_eq!(registry.contexts[0].registrations.len(), 2);
    }
}
