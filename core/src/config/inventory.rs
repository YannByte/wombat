//! Prepare a pinned physical inventory and bind observed source facts without loading inference.
use super::*;
use std::path::Path;
pub(crate) fn prepare_observed(
    r: &Request,
    snapshot: Option<Arc<Snapshot>>,
    history_status: String,
    native: Option<&hooks::Capture>,
    native_skills: Option<&review::Capture>,
) -> Result<View> {
    // Serialize inventory refreshes, not usage queries or reads of retained views.
    static REFRESH: Mutex<()> = Mutex::new(());
    let _refresh = REFRESH.lock().unwrap_or_else(|e| e.into_inner());
    let roots = r.roots.clone().unwrap_or_default();
    let discovery = crate::adapters::codex::CodexAdapter.discover(&DiscoveryRequest {
        roots: roots.iter().map(PathBuf::from).collect(),
    });
    let sources = discovery.sources;
    let projects = scope::projects(r)?;
    let checked = Utc::now().to_rfc3339();
    let mut inventory = scan::scan(&sources, &projects, &checked);
    inventory
        .issues
        .extend(discovery.issues.into_iter().map(|issue| {
            Issue {
                code: issue.code,
                path: sources
                    .iter()
                    .find(|source| Some(&source.id) == issue.source_instance_id.as_ref())
                    .map(|source| source.root.clone()),
            }
        }));
    scan::append_native_hooks(&mut inventory, native, &sources, &projects, &checked);
    scan::append_native_skills(&mut inventory, native_skills, &sources, &projects, &checked);
    if let Some(code) = native_skills.and_then(|c| c.error_code.as_ref()) {
        inventory.issues.push(Issue {
            code: code.clone(),
            path: None,
        });
    }
    let hook_registry = hooks::bind(native, &inventory.items, &projects);
    inventory.issues.retain(|issue| {
        issue.code != "hookEffectiveRegistryUnavailable"
            || !matches!(hook_registry.status, HookRegistryStatus::Observed)
    });
    let config_collection = ConfigCollection::capture(
        &inventory.issues,
        sources.iter().map(|source| source.root.clone()).collect(),
    );
    let mut analysis = analysis::analyze_authorized(
        &inventory.items,
        &projects,
        &inventory
            .authorized_roots
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
    );
    hook_targets::analyze(
        &inventory.items,
        &projects,
        &inventory
            .authorized_roots
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        &hook_registry,
        native,
        &mut analysis,
    );
    inventory.issues.extend(analysis.issues.clone());
    let cache_key = crate::hash(serde_json::to_vec(&(
        sources.iter().map(|s| &s.id).collect::<Vec<_>>(),
        &projects,
    ))?);
    let cache = crate::storage::data_home()?
        .join("config-v2")
        .join(format!("{cache_key}.json"));
    // Only safe metadata is retained, never config text, env values or command arguments.
    if let Ok(metadata) = std::fs::metadata(&cache)
        && metadata.len() <= 16 * 1024 * 1024
        && let Ok(bytes) = std::fs::read(&cache)
        && bytes.len() <= 16 * 1024 * 1024
        && let Ok(prior) = serde_json::from_slice::<Vec<Item>>(&bytes)
    {
        let ids = inventory
            .items
            .iter()
            .map(|i| i.id.clone())
            .collect::<BTreeSet<_>>();
        for mut item in prior {
            if ids.contains(&item.id) {
                continue;
            }
            item.stale = inventory.issues.iter().any(|issue| {
                issue
                    .path
                    .as_ref()
                    .is_some_and(|p| PathBuf::from(&item.path).starts_with(p))
                    && issue.code != "effectiveConfigUnknown"
            });
            // Native-listed declarations may disappear from an observation while the file
            // remains. Without a successful source read, absence is not removal evidence.
            if item.kind == Kind::Hook
                && !inventory.read_paths.contains(&item.path)
                && inventory
                    .authorized_roots
                    .iter()
                    .any(|root| Path::new(&item.path).starts_with(root))
                && !std::fs::symlink_metadata(&item.path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                item.stale = true;
            }
            item.current = item.stale;
            item.counts = Counts::default();
            item.usage = None;
            item.related_turns = 0;
            item.observation = Observation::Unknown;
            item.context_footprint = None;
            if inventory.items.len() < 20_000 {
                inventory.items.push(item);
            }
        }
    }
    inventory.items.sort_by(|a, b| a.id.cmp(&b.id));
    let metadata = serde_json::to_vec(&inventory.items)?;
    if metadata.len() > 16 * 1024 * 1024 {
        inventory.issues.push(Issue {
            code: "resourceLimited".into(),
            path: None,
        });
    } else if crate::storage::atomic_write(&cache, &metadata).is_err() {
        inventory.issues.push(Issue {
            code: "inventoryCacheUnavailable".into(),
            path: None,
        });
    }
    // Native inheritance augments only this view, after the safe inventory cache was written.
    if r.review || native_skills.is_some() {
        review::bind(&mut inventory.items, native_skills, &projects);
    }
    let indexes: BTreeMap<_, _> = inventory
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id.clone(), index))
        .collect();
    for context in &hook_registry.contexts {
        for registration in &context.registrations {
            if let Some(index) = indexes.get(&registration.item_id) {
                let item = &mut inventory.items[*index];
                if !item.authorized_projects.contains(&context.project) {
                    item.authorized_projects.push(context.project.clone());
                    item.authorized_projects.sort();
                }
            }
        }
    }
    let mut view = View {
        snapshot,
        items: inventory.items,
        issues: inventory.issues,
        projects,
        roots,
        project_roots: r.project_roots.clone().unwrap_or_default(),
        revision: String::new(),
        checked,
        history_status,
        analysis,
        hook_registry,
        config_collection,
        observation_versions: OnceLock::new(),
    };
    view.revision = view.observation_versions()?.combined_revision()?;
    Ok(view)
}
