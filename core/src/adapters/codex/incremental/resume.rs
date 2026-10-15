//! Persist only parser-specific state. Unchanged canonical facts are references,
//! not another ledger; resume never reconstructs accounting from a query result.
use super::*;

pub(super) struct Delta {
    pub measurements: BTreeSet<String>,
    pub operations: BTreeSet<String>,
    pub retired: bool,
    pub targets: Vec<String>,
    pub raw: bool,
}
#[derive(Serialize, Deserialize)]
struct CandidateState {
    direct: bool,
    cumulative: Option<u64>,
    interval_start: Option<u64>,
    fingerprint: String,
    measurement: Option<Arc<Measurement>>,
}
#[derive(Serialize, Deserialize)]
struct Header {
    source: String,
    measurements: usize,
    operations: usize,
    associations_available: bool,
    raw_measurements: bool,
    /// Maps below are parser inputs, before project conflict and fork projection.
    threads: BTreeMap<String, Thread>,
    turns: BTreeMap<String, Turn>,
    parents: HashMap<String, String>,
    projects: BTreeMap<String, BTreeSet<String>>,
    conflicts: BTreeSet<String>,
}
fn scope(source: &SourceInstance) -> String {
    format!("{}:resume", fact_scope(source))
}
pub(super) fn validate(db: &Connection, source: &SourceInstance) -> Result<bool> {
    let scope = scope(source);
    if !crate::live_index::has_scope(db, &scope)? {
        return Ok(false);
    }
    match crate::live_index::scalar(db, &scope, "version")?.and_then(|v| v.as_u64()) {
        Some(1) => Ok(true),
        Some(_) => Err(crate::dto::operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported parser resume state",
        )),
        None => Err(crate::dto::operation_error(
            "INDEX_UNAVAILABLE",
            "Parser resume version missing",
        )),
    }
}
pub(super) fn weight(db: &Connection, source: &str) -> Result<u64> {
    let parser = format!("parser:{source}:{VERSION}:1:facts:resume");
    let projection = format!("projection:{source}");
    let bytes: i64 = db.query_row("SELECT (SELECT COALESCE(SUM(f.bytes),0) FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?1 OR b.scope=?2)+(SELECT COALESCE(SUM(length(e.payload)),0) FROM entries e JOIN buckets b ON b.id=e.bucket WHERE b.scope=?1)",rusqlite::params![parser,projection],|row|row.get(0))?;
    Ok(bytes.max(0) as u64)
}

pub(super) fn save(
    db: &Connection,
    source: &SourceInstance,
    facts: &Facts,
    projection: &Collected,
    delta: Option<&Delta>,
) -> Result<()> {
    let scope = scope(source);
    crate::live_index::put(db, &scope, "version", "", &1_u32)?;
    let prior = crate::live_index::scalar(db, &scope, "header")?;
    let raw = delta.map_or_else(
        || !facts.parents.is_empty() || facts.measurements.values().any(|c| !c.direct),
        |d| d.raw,
    );
    let delta = delta.filter(|_| {
        prior.as_ref().is_some_and(|h| {
            h["raw_measurements"].as_bool() == Some(raw)
                && h["parents"]
                    .as_object()
                    .is_some_and(|p| p.is_empty() == facts.parents.is_empty())
        })
    });
    let selected: Vec<_> = match delta {
        Some(d) => d
            .measurements
            .iter()
            .filter_map(|id| facts.measurements.get_key_value(id))
            .collect(),
        None => facts.measurements.iter().collect(),
    };
    let candidates: Vec<_> = selected
        .into_iter()
        .map(|(id, c)| {
            (
                id.as_str(),
                CandidateState {
                    direct: c.direct,
                    cumulative: c.cumulative,
                    interval_start: c.interval_start,
                    fingerprint: c.fingerprint.clone(),
                    measurement: if !raw
                        && projection
                            .measurements
                            .binary_search_by(|m| m.id.cmp(id))
                            .ok()
                            .is_some_and(|n| projection.measurements[n] == c.measurement)
                    {
                        None
                    } else {
                        Some(Arc::clone(&c.measurement))
                    },
                },
            )
        })
        .collect();
    crate::live_index::put_packed(
        db,
        &scope,
        "candidates",
        candidates.iter().map(|(id, c)| (*id, c)),
    )?;
    if delta.is_none() {
        crate::live_index::retain_packed(
            db,
            &scope,
            "candidates",
            facts.measurements.keys().map(String::as_str),
        )?;
    }
    // Operations before inherited-copy removal can differ from canonical output.
    let selected: Vec<_> = match delta {
        Some(d) => d
            .operations
            .iter()
            .filter_map(|id| facts.operations.get_key_value(id))
            .collect(),
        None => facts.operations.iter().collect(),
    };
    let operations: Vec<_> = selected
        .into_iter()
        .map(|(id, o)| {
            (
                id.as_str(),
                if facts.parents.is_empty()
                    && projection
                        .operations
                        .binary_search_by(|p| p.id.cmp(id))
                        .ok()
                        .is_some_and(|n| projection.operations[n] == *o)
                {
                    None
                } else {
                    Some(Arc::clone(o))
                },
            )
        })
        .collect();
    crate::live_index::put_packed(
        db,
        &scope,
        "operations",
        operations.iter().map(|(id, o)| (*id, o)),
    )?;
    if delta.is_none_or(|d| d.retired) {
        crate::live_index::retain_packed(
            db,
            &scope,
            "operations",
            facts.operations.keys().map(String::as_str),
        )?;
    }
    let changed = delta
        .filter(|_| {
            prior
                .as_ref()
                .is_some_and(|h| h["associations_available"] == true)
        })
        .map(|d| d.targets.as_slice());
    facts.save_associations(db, &scope, changed)?;
    let aliases: Vec<_> = if facts.association_groups.is_none() {
        facts.operation_aliases.iter().collect()
    } else {
        vec![]
    };
    crate::live_index::put_packed(
        db,
        &scope,
        "aliases",
        aliases.iter().map(|(id, aliases)| (id.as_str(), *aliases)),
    )?;
    crate::live_index::retain_packed(
        db,
        &scope,
        "aliases",
        aliases.iter().map(|(id, _)| id.as_str()),
    )?;
    crate::live_index::put(
        db,
        &scope,
        "header",
        "",
        &Header {
            source: source.id.clone(),
            measurements: facts.measurements.len(),
            operations: facts.operations.len(),
            associations_available: facts.association_groups.is_some(),
            raw_measurements: raw,
            threads: facts.threads.clone(),
            turns: facts.turns.clone(),
            parents: facts.parents.clone(),
            projects: facts.projects.clone(),
            conflicts: facts.measurement_conflicts.clone(),
        },
    )?;
    Ok(())
}

pub(super) fn load(db: &Connection, source: &SourceInstance) -> Result<Option<Facts>> {
    if !validate(db, source)? {
        return Ok(None);
    }
    let scope = scope(source);
    let Some(header) = crate::live_index::scalar(db, &scope, "header")? else {
        return Ok(None);
    };
    let header: Header = serde_json::from_value(header)?;
    anyhow::ensure!(header.source == source.id, "resume source mismatch");
    let projection = crate::live::load_collected_except(db, &source.id, true, false, false)?
        .ok_or_else(|| anyhow::anyhow!("resume projection missing"))?;
    let mut measurements: BTreeMap<_, _> = projection
        .measurements
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect();
    let mut operations: BTreeMap<_, _> = projection
        .operations
        .into_iter()
        .map(|o| (o.id.clone(), o))
        .collect();
    let mut facts = Facts {
        threads: header.threads,
        turns: header.turns,
        parents: header.parents,
        projects: header.projects,
        measurement_conflicts: header.conflicts,
        streaming: true,
        association_groups: header.associations_available.then(BTreeMap::new),
        ..Facts::default()
    };
    for selected in ["candidates", "operations"] {
        crate::live_index::each_located(db, &scope, Some(selected), |field, id, payload, _| {
            match field {
                "header" => {}
                "candidates" => {
                    let c: CandidateState = serde_json::from_str(payload)?;
                    let measurement = c
                        .measurement
                        .or_else(|| measurements.remove(id))
                        .ok_or_else(|| anyhow::anyhow!("resume measurement reference missing"))?;
                    anyhow::ensure!(
                        measurement.id == id
                            && measurement.source_instance_id.as_ref() == source.id,
                        "resume measurement identity mismatch"
                    );
                    facts.measurements.insert(
                        id.into(),
                        Candidate {
                            measurement,
                            direct: c.direct,
                            cumulative: c.cumulative,
                            interval_start: c.interval_start,
                            fingerprint: c.fingerprint,
                        },
                    );
                }
                "operations" => {
                    let operation = serde_json::from_str::<Option<Arc<Operation>>>(payload)?
                        .or_else(|| operations.remove(id))
                        .ok_or_else(|| anyhow::anyhow!("resume operation reference missing"))?;
                    anyhow::ensure!(operation.id == id, "resume operation identity mismatch");
                    facts.operations.insert(id.into(), operation);
                }
                // Association references resolve only after the operation directory.
                "associations" => {}
                _ => anyhow::bail!("unsupported resume field"),
            }
            Ok(())
        })?;
    }
    drop(measurements);
    drop(operations);
    anyhow::ensure!(
        facts.measurements.len() == header.measurements
            && facts.operations.len() == header.operations,
        "resume state incomplete"
    );
    for (id, candidate) in &mut facts.measurements {
        let m = Arc::make_mut(&mut candidate.measurement);
        facts.strings.measurement(m);
        anyhow::ensure!(*id == m.id, "resume candidate mismatch");
    }
    for operation in facts.operations.values_mut() {
        facts.strings.operation(Arc::make_mut(operation));
    }
    crate::live_index::each_located(db, &scope, Some("associations"), |_, _, payload, _| {
        facts.restore_association(payload)
    })?;
    if header.associations_available {
        facts.restore_aliases();
    } else {
        let mut aliases = BTreeMap::new();
        crate::live_index::each_located(db, &scope, Some("aliases"), |_, id, payload, _| {
            anyhow::ensure!(
                facts.operations.contains_key(id),
                "resume alias operation missing"
            );
            aliases.insert(id.to_owned(), serde_json::from_str(payload)?);
            Ok(())
        })?;
        facts.operation_aliases = Arc::new(aliases);
    }
    facts.title_observations = projection
        .title_observations
        .into_iter()
        .map(|t| (t.thread_id.clone(), t))
        .collect();
    Ok(Some(facts))
}
