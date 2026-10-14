//! First-value snapshots never enter the durable projection or seed its caches.
use super::*;

pub(super) fn is_initial(snapshot: &Snapshot) -> bool {
    snapshot
        .manifest
        .issues
        .iter()
        .any(|issue| issue.code == adapters::codex::preview::ISSUE)
}

pub(super) fn prepare(key: &str, roots: &[String]) -> Result<Option<Arc<Snapshot>>> {
    let collected = adapters::codex::preview::collect(sources(roots));
    if collected.threads.is_empty() {
        return Ok(None);
    }
    Ok(Some(Arc::new(crate::usage_store::memory(
        collected,
        format!("live:{key}:{}", uuid::Uuid::new_v4()),
        crate::pricing_sync::current()?,
        None,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_views_are_readable_but_neither_committed_cache_nor_export() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sessions")).unwrap();
        fs::write(
            root.path().join("sessions/one.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"task\"}}\n",
        )
        .unwrap();
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let id = format!("live:{key}:preview");
        let view = Arc::new(
            crate::usage_store::memory(
                adapters::codex::preview::collect(sources(&roots)),
                id.clone(),
                crate::pricing_sync::current_at(root.path()).unwrap(),
                None,
            )
            .unwrap(),
        );
        let mut entry = Entry::new(roots.clone());
        entry.syncing = true;
        entry.views.push_back((Instant::now(), view));
        let shared: Shared = Arc::new((Mutex::new(BTreeMap::from([(key, entry)])), Condvar::new()));
        let (jobs, _receiver) = mpsc::sync_channel(32);
        let request = |action: &str, snapshot: Option<&str>, mode: Mode| {
            Request {
            query: serde_json::from_value(json!({"action":action,"roots":roots,"snapshotId":snapshot,"scope":{"allTime":true}})).unwrap(),
            mode, verify:false, wait_ms:None, rebuild:false, include_history:false, also:vec![],
        }
        };
        let configs = Mutex::new(crate::config::Store::default());
        let result = query(
            request("threads", Some(&id), Mode::Cached),
            &shared,
            &jobs,
            &configs,
        )
        .unwrap();
        assert!(result.freshness.initial_scan);
        assert_eq!(result.result.items.len(), 1);
        assert_eq!(result.result.quality.status, "partial");
        let usage = query(
            request("usage", Some(&id), Mode::Cached),
            &shared,
            &jobs,
            &configs,
        )
        .unwrap();
        let facets = usage.result.facets.unwrap();
        assert_eq!(facets.agents, ["codex"]);
        assert!(facets.has_unassigned);
        for (request, expected) in [
            (request("usage", None, Mode::Cached), "NO_SNAPSHOT"),
            (
                request("refresh", Some(&id), Mode::Auto),
                "INVALID_ARGUMENT",
            ),
        ] {
            let error = query(request, &shared, &jobs, &configs).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::dto::OperationError>()
                    .unwrap()
                    .code,
                expected
            );
        }
        assert!(!root.path().join("index.sqlite").exists());
    }
}
