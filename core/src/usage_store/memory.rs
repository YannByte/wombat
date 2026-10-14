//! Compact live snapshots share immutable facts and index turn membership.
use super::*;
pub(crate) fn memory(
    collected: Collected,
    id: String,
    prices: crate::pricing_sync::Response,
    prior: Option<&Snapshot>,
) -> Result<Snapshot> {
    memory_inner(collected, id, prices, prior, None, None)
}
pub(crate) fn memory_cold_operations(
    collected: Collected,
    id: String,
    prices: crate::pricing_sync::Response,
    events: EventIndex,
    operations: ColdOperations,
) -> Result<Snapshot> {
    anyhow::ensure!(
        collected.events.is_empty() && collected.operations.is_empty(),
        "cold view must not retain detail payloads"
    );
    memory_inner(
        collected,
        id,
        prices,
        None,
        Some((events, None)),
        Some(operations),
    )
}
fn memory_inner(
    collected: Collected,
    id: String,
    prices: crate::pricing_sync::Response,
    prior: Option<&Snapshot>,
    prepared_events: Option<(EventIndex, Option<super::events::EventBuckets>)>,
    cold_operations: Option<ColdOperations>,
) -> Result<Snapshot> {
    validate_watermarks(&collected.watermarks)?;
    validate_title_observations(&collected.title_observations, &collected.threads)?;
    let mut pool = super::price_pool::PricePool::new(&prices);
    let reusable = prior
        .filter(|s| s.manifest.price_catalog_hash == prices.catalog_hash)
        .and_then(|s| s.live_rows.as_ref())
        .map(|r| r.as_slice());
    let mut facts = collected.measurements;
    facts.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    let mut previous = reusable.into_iter().flatten().peekable();
    let mut rows = Vec::with_capacity(facts.len());
    let mut memory_turns: BTreeMap<(String, String), MemoryTurn> = BTreeMap::new();
    for fact in facts {
        while previous.peek().is_some_and(|r| r.fact.id < fact.id) {
            previous.next();
        }
        let prior = previous
            .peek()
            .copied()
            .filter(|r| r.fact.id == fact.id)
            .filter(|r| Arc::ptr_eq(&r.fact, &fact) || r.fact == fact);
        let row = match prior {
            Some(row) => {
                pool.remember(&row.fact, &row.price);
                Arc::clone(row)
            }
            None => {
                let price = pool.price(&fact);
                Arc::new(PricedMeasurement { fact, price })
            }
        };
        rows.push(row);
    }
    drop(pool);
    // One membership index per immutable view; a project query touches its rows only.
    let owners: std::collections::HashMap<_, _> = collected
        .threads
        .iter()
        .map(|thread| (thread.id.as_str(), thread.project.as_deref()))
        .collect();
    let mut memberships = std::collections::HashMap::<Option<&str>, Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        let project = row
            .fact
            .thread_id
            .as_deref()
            .and_then(|id| owners.get(id))
            .copied()
            .flatten();
        memberships.entry(project).or_default().push(index);
    }
    let project_rows = memberships
        .into_iter()
        .map(|(project, rows)| (project.map(str::to_owned), rows))
        .collect();
    drop(owners);

    if rows
        .windows(2)
        .any(|pair| pair[0].fact.id == pair[1].fact.id)
    {
        return Err(operation_error("INVALID_FACTS", "计量身份重复"));
    }
    for (index, row) in rows.iter().enumerate() {
        if let Some(thread) = &row.fact.thread_id {
            memory_turns
                .entry((
                    thread.to_string(),
                    row.fact
                        .turn_id
                        .as_deref()
                        .unwrap_or("unassigned")
                        .to_owned(),
                ))
                .or_default()
                .measurements
                .push(index);
        }
    }
    crate::usage_app::summarize(&rows.iter().map(Arc::as_ref).collect::<Vec<_>>())?;
    let summary_index = crate::usage_app::SummaryIndex::build_reusing(
        &rows.iter().map(Arc::as_ref).collect::<Vec<_>>(),
        prior.map(|s| s.summary_index.as_ref()),
    )?;
    let mut unassigned_uses = BTreeMap::<String, UnassignedUseRecords>::new();
    if let Some(operations) = cold_operations {
        unassigned_uses = operations.unassigned;
        for (key, locations) in operations.turns {
            memory_turns.entry(key).or_default().cold_operations = locations;
        }
    }
    for op in collected.operations {
        if op.turn_id.as_deref().is_none_or(|id| id.is_empty()) {
            unassigned_uses
                .entry(op.thread_id.to_string())
                .or_default()
                .observe(&op)?;
        }
        memory_turns
            .entry((
                op.thread_id.to_string(),
                op.turn_id.as_deref().unwrap_or("unassigned").to_owned(),
            ))
            .or_default()
            .operations
            .push(op);
    }
    let turns: BTreeMap<_, _> = collected
        .turns
        .into_iter()
        .map(|t| ((t.thread_id.clone(), t.id.clone()), t))
        .collect();
    for key in turns.keys() {
        memory_turns.entry(key.clone()).or_default();
    }
    let mut by_thread: BTreeMap<String, BTreeMap<String, TurnEntry>> = BTreeMap::new();
    for (thread, turn) in memory_turns.keys() {
        by_thread.entry(thread.to_string()).or_default().insert(
            turn.clone(),
            TurnEntry {
                turn: turns.get(&(thread.to_string(), turn.clone())).cloned(),
                slice: Slice {
                    offset: 0,
                    length: 0,
                    sha256: String::new(),
                },
            },
        );
    }
    let threads = collected
        .threads
        .into_iter()
        .map(|thread| {
            let turns = by_thread.remove(&thread.id).unwrap_or_default();
            Arc::new(ThreadEntry {
                unassigned_uses: unassigned_uses.remove(&thread.id).unwrap_or_default(),
                thread,
                turns,
                file: file_ref("live", &[]),
            })
        })
        .collect();
    if !by_thread.is_empty() {
        return Err(operation_error("INVALID_FACTS", "存在没有对话元数据的记录"));
    }
    let (events, live_events) = match prepared_events {
        Some(events) => events,
        None => {
            let (index, buckets) = super::events::reuse_memory_events(collected.events, prior)?;
            (index, Some(buckets))
        }
    };
    let observation_versions = crate::observation_versions::SnapshotObservationVersions::current();
    let manifest = Manifest {
        schema_version: 4,
        observation_versions,
        title_observations: collected.title_observations,
        snapshot_ref: SnapshotRef {
            snapshot_id: id,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        price_revision: prices.catalog.revision,
        price_catalog_hash: prices.catalog_hash,
        sources: collected.sources,
        watermarks: collected.watermarks,
        issues: collected.issues,
        ledger: file_ref("live", &[]),
        events,
        threads,
    };
    Ok(Snapshot {
        lazy_details: None,
        summary_index: Arc::new(summary_index),
        publication_change: None,
        project_loads: vec![],
        project_rows: Some(Arc::new(project_rows)),
        timing_cache: Mutex::default(),
        query_cache: Mutex::default(),
        query_work: Arc::default(),
        manifest,
        directory: PathBuf::new(),
        live_rows: Some(Arc::new(rows)),
        live_events,
        cold_events: None,
        memory_turns: Some(Arc::new(memory_turns)),
    })
}
