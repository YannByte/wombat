//! A fork retains upstream turn/call identities; merge replays into their oldest recorded ancestor.
use super::*;
impl Facts {
    pub(in crate::adapters::codex) fn project_operations(
        &self,
        metadata: &mut Facts,
        forest: &session_relations::SessionRelations<'_>,
        report: &mut SourceReport,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<Vec<Arc<Operation>>> {
        let check = crate::operation_association::check;
        check(cancelled)?;
        if self.parents.is_empty() {
            return self
                .operations
                .values()
                .map(|operation| {
                    check(cancelled)?;
                    Ok(Arc::clone(operation))
                })
                .collect();
        }
        let mut entries = Vec::new();
        for op in self.operations.values() {
            check(cancelled)?;
            let turn = op.turn_id.as_deref().and_then(|id| self.turns.get(id));
            if let Some(turn) = turn
                && let Some(aliases) = self.operation_aliases.get(&op.id)
            {
                for alias in aliases.iter() {
                    check(cancelled)?;
                    entries.push((
                        (turn.upstream_id.as_str(), alias),
                        op.thread_id.as_ref(),
                        op.id.as_str(),
                    ));
                }
            }
        }
        // Every edge already proves native ancestry and a typed alias. A late
        // dual-alias observation can join multiple recorded ancestor groups.
        let replay = replay_plan(
            forest
                .replays_cancellable(entries.into_iter(), cancelled)?
                .into_iter()
                .map(|(a, b)| Ok((a.to_owned(), b.to_owned()))),
            |id| Ok(owner_key(&self.operations[id], forest)),
            cancelled,
        )?;
        let mut retired = BTreeSet::new();
        let mut replacements = BTreeMap::new();
        let mut remaining = replay.as_slice();
        while let Some((ancestor, _)) = remaining.first() {
            check(cancelled)?;
            let count = remaining.partition_point(|(owner, _)| owner == ancestor);
            let (group, rest) = remaining.split_at(count);
            remaining = rest;
            let Some(parent) = self
                .operations
                .get(ancestor)
                .filter(|_| !retired.contains(ancestor.as_str()))
            else {
                continue;
            };
            let mut parent = parent.as_ref().clone();
            for (_, id) in group {
                check(cancelled)?;
                let Some(child) = self
                    .operations
                    .get(id)
                    .filter(|_| retired.insert(id.as_str()))
                else {
                    continue;
                };
                merge_replay(&mut parent, child, metadata, report, cancelled)?;
            }
            finish_replay(&mut parent, cancelled)?;
            // This is a disposable projection. Parsed facts and their native aliases stay untouched.
            replacements.insert(ancestor.as_str(), Arc::new(parent));
        }
        check(cancelled)?;
        let mut projected = Vec::new();
        for (id, operation) in &self.operations {
            check(cancelled)?;
            if !retired.contains(id.as_str()) {
                projected.push(
                    replacements
                        .remove(id.as_str())
                        .unwrap_or_else(|| Arc::clone(operation)),
                );
            }
        }
        Ok(projected)
    }
}

pub(in crate::adapters::codex) type OwnerKey =
    (Option<usize>, Option<Arc<str>>, Option<u64>, String);
pub(in crate::adapters::codex) fn owner_key(
    operation: &Operation,
    forest: &session_relations::SessionRelations<'_>,
) -> OwnerKey {
    let evidence = operation.evidence.first();
    (
        forest.owner_order(&operation.thread_id),
        evidence.map(|e| Arc::clone(&e.file)),
        evidence.map(|e| e.line),
        operation.id.clone(),
    )
}
/// Resolve only proven replay links. Operation bodies are supplied by the
/// caller on demand; unrelated operations never enter this graph.
pub(in crate::adapters::codex) fn replay_plan(
    links: impl IntoIterator<Item = anyhow::Result<(String, String)>>,
    mut key: impl FnMut(&str) -> anyhow::Result<OwnerKey>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Vec<(String, String)>> {
    let check = crate::operation_association::check;
    let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
    for link in links {
        check(cancelled)?;
        let (a, b) = link?;
        edges.entry(a.clone()).or_default().insert(b.clone());
        edges.entry(b).or_default().insert(a);
    }
    let mut seen = BTreeSet::new();
    let mut replay = Vec::new();
    for first in edges.keys() {
        check(cancelled)?;
        if seen.contains(first) {
            continue;
        }
        let mut pending = vec![first];
        let mut group = Vec::new();
        while let Some(id) = pending.pop() {
            check(cancelled)?;
            if !seen.insert(id) {
                continue;
            }
            group.push(id);
            pending.extend(edges[id].iter().filter(|next| !seen.contains(next)));
        }
        let mut owner = first;
        let mut owner_key = key(owner)?;
        for id in &group {
            check(cancelled)?;
            let candidate = key(id)?;
            if candidate < owner_key {
                owner = id;
                owner_key = candidate;
            }
        }
        for id in group {
            if id != owner {
                replay.push((owner.to_owned(), id.to_owned()));
            }
        }
    }
    check(cancelled)?;
    replay.sort_unstable();
    check(cancelled)?;
    Ok(replay)
}
pub(in crate::adapters::codex) fn merge_replay(
    parent: &mut Operation,
    child: &Operation,
    metadata: &mut Facts,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    let mut observed = child.clone();
    observed.id.clone_from(&parent.id);
    observed.thread_id.clone_from(&parent.thread_id);
    observed.turn_id.clone_from(&parent.turn_id);
    metadata.observe_operation(&observed);
    merge_metadata(parent, &mut observed, report);
    for evidence in observed.evidence {
        crate::operation_association::check(cancelled)?;
        parent.evidence.push(evidence);
    }
    Ok(())
}
pub(in crate::adapters::codex) fn finish_replay(
    parent: &mut Operation,
    cancelled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    crate::operation_association::check(cancelled)?;
    parent
        .evidence
        .sort_unstable_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    crate::operation_association::check(cancelled)?;
    parent.evidence.dedup();
    Ok(())
}
