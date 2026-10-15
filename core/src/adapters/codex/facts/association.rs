//! Retain canonical components, not their raw phase observations. The same
//! resolver and merge rules serve full replay and one-target archive reduction.
use super::*;
use crate::operation_association::OwnedAlias;

#[derive(Serialize, Deserialize)]
pub(in crate::adapters::codex) struct Prepared {
    preferred: String,
    fallback: String,
    anchor: (Arc<str>, Arc<str>, Arc<str>, u64, u32),
    scope: (Arc<str>, Option<Arc<str>>, Option<Arc<str>>),
    #[serde(skip)]
    operation: Option<Arc<Operation>>,
    aliases: Arc<[OwnedAlias]>,
    issues: Vec<Issue>,
}
impl Facts {
    pub(in crate::adapters::codex) fn save_associations(
        &self,
        db: &rusqlite::Connection,
        scope: &str,
        changed: Option<&[String]>,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Stored<'a> {
            target: &'a str,
            #[serde(flatten)]
            group: &'a Prepared,
            operation_id: Option<&'a str>,
        }
        let selected: Vec<_> = match (&self.association_groups, changed) {
            (Some(groups), Some(changed)) => changed
                .iter()
                .filter_map(|target| groups.get_key_value(target))
                .collect(),
            (Some(groups), None) => groups.iter().collect(),
            (None, _) => vec![],
        };
        let rows: Vec<_> = selected
            .into_iter()
            .flat_map(|(target, groups)| {
                let prefix = crate::hash(target);
                groups.iter().enumerate().map(move |(n, group)| {
                    (
                        format!("{prefix}:{n}"),
                        Stored {
                            target,
                            group,
                            operation_id: group.operation.as_ref().map(|o| o.id.as_str()),
                        },
                    )
                })
            })
            .collect();
        crate::live_index::put_packed(
            db,
            scope,
            "associations",
            rows.iter().map(|(id, row)| (id.as_str(), row)),
        )?;
        if let Some(changed) = changed {
            for target in changed {
                let prefix = format!("{}:", crate::hash(target));
                crate::live_index::retain_packed_prefix(
                    db,
                    scope,
                    "associations",
                    Some(&prefix),
                    rows.iter()
                        .filter(|(id, _)| id.starts_with(&prefix))
                        .map(|(id, _)| id.as_str()),
                )?;
            }
            return Ok(());
        }
        crate::live_index::retain_packed(
            db,
            scope,
            "associations",
            rows.iter().map(|(id, _)| id.as_str()),
        )
    }
    pub(in crate::adapters::codex) fn restore_association(
        &mut self,
        payload: &str,
    ) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct Stored {
            target: String,
            #[serde(flatten)]
            group: Prepared,
            operation_id: Option<String>,
        }
        let mut stored: Stored = serde_json::from_str(payload)?;
        if let Some(id) = stored.operation_id {
            let operation = self
                .operations
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("resume operation reference missing"))?;
            stored.group.operation = Some(Arc::clone(operation));
        }
        self.association_groups
            .get_or_insert_default()
            .entry(stored.target)
            .or_default()
            .push(stored.group);
        Ok(())
    }
    pub(in crate::adapters::codex) fn restore_aliases(&mut self) {
        self.operation_aliases = Arc::new(
            self.association_groups
                .iter()
                .flat_map(|g| g.values())
                .flatten()
                .filter_map(|g| {
                    g.operation
                        .as_ref()
                        .map(|o| (o.id.clone(), Arc::clone(&g.aliases)))
                })
                .collect(),
        );
    }
    #[cfg(test)]
    pub(in crate::adapters::codex) fn associations_share_canonical_allocations(&self) -> bool {
        self.association_groups.as_ref().is_some_and(|groups| {
            groups.values().flatten().all(|group| {
                group.operation.as_ref().is_none_or(|operation| {
                    self.operations
                        .get(&operation.id)
                        .is_some_and(|canonical| Arc::ptr_eq(operation, canonical))
                })
            })
        })
    }
    pub(in crate::adapters::codex) fn prepare_associations<'a>(
        events: impl IntoIterator<Item = &'a crate::session_events::Event>,
        strings: &mut super::super::super::shared_strings::FactStrings,
        report: &mut SourceReport,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<Vec<Prepared>> {
        let check = crate::operation_association::check;
        let groups = crate::operation_association::resolve_groups(
            events
                .into_iter()
                .filter(|event| crate::operation_association::is_projection_input(event)),
            cancelled,
        )?;
        let mut prepared = Vec::with_capacity(groups.len());
        for group in groups {
            check(cancelled)?;
            // A phase-only component has no domain operation. The archive keeps
            // its aliases and phases for timing and future affected-target replay.
            if group.operation_events.is_empty() {
                continue;
            }
            debug_assert!(!group.id.is_empty());
            let position = group.anchor.position();
            let file = match group.anchor.payload() {
                crate::session_events::Payload::Operation { value, .. } => {
                    value.evidence.first().map(|e| Arc::clone(&e.file))
                }
                _ => None,
            }
            .unwrap_or_else(|| Arc::clone(&position.file_id));
            let target = group.anchor.shared_target();
            let fallback =
                crate::operation_association::fallback_id(group.scope, group.anchor.position());
            let preferred = match group.anchor.payload() {
                crate::session_events::Payload::Operation { value, .. } if !value.id.is_empty() => {
                    value.id.clone()
                }
                _ => fallback.clone(),
            };
            let identity = Prepared {
                preferred,
                fallback,
                anchor: (
                    Arc::clone(&position.source_instance_id),
                    file,
                    Arc::clone(&position.generation),
                    position.byte_offset,
                    position.ordinal,
                ),
                scope: (Arc::clone(&position.source_instance_id), target.0, target.1),
                operation: None,
                aliases: Arc::from([]),
                issues: vec![],
            };
            let first_issue = report.issues.len();
            let mut observed = group.operation_events.into_iter();
            let first = observed.next().expect("non-empty operation component");
            let crate::session_events::Payload::Operation { value, .. } = first.payload() else {
                unreachable!()
            };
            let mut operation = value.as_ref().clone();
            operation.id.clone_from(&identity.preferred);
            for event in observed {
                crate::operation_association::check(cancelled)?;
                let crate::session_events::Payload::Operation { value, .. } = event.payload()
                else {
                    unreachable!()
                };
                let mut incoming = value.as_ref().clone();
                operations::merge_metadata(&mut operation, &mut incoming, report);
                for evidence in incoming.evidence {
                    check(cancelled)?;
                    operation.evidence.push(evidence);
                }
            }
            // Identity/phase association owns these contradictions. A missing field
            // does not contradict a recorded target; merged metadata cannot refill it.
            if group.target_conflict {
                operation.kind = "mcpConflict".into();
                operation.server = None;
                operation.tool = None;
                issue(
                    report,
                    "operationIdentityConflict",
                    "同一 MCP 调用的来源身份冲突",
                    operation.evidence.first().cloned(),
                );
            }
            if group.outcome_conflict {
                operation.outcome_conflict = true;
                operation.exit_code = None;
            }
            crate::operation_association::check(cancelled)?;
            operation
                .evidence
                .sort_unstable_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            check(cancelled)?;
            operation.evidence.dedup();
            check(cancelled)?;
            strings.operation(&mut operation);

            let aliases: Arc<[OwnedAlias]> = group
                .aliases
                .into_iter()
                .map(|alias| alias.owned())
                .collect();
            prepared.push(Prepared {
                operation: Some(Arc::new(operation)),
                aliases,
                issues: report.issues.split_off(first_issue),
                ..identity
            });
        }
        Ok(prepared)
    }

    pub(in crate::adapters::codex) fn publish_associations(
        &mut self,
        report: &mut SourceReport,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<()> {
        let check = crate::operation_association::check;
        check(cancelled)?;
        let mut ordered = Vec::new();
        for (target, groups) in self
            .association_groups
            .as_mut()
            .expect("prepared associations")
            .iter_mut()
        {
            for group in groups {
                check(cancelled)?;
                ordered.push((target, group));
            }
        }
        ordered
            .sort_unstable_by(|(_, a), (_, b)| (&a.anchor, &a.scope).cmp(&(&b.anchor, &b.scope)));
        check(cancelled)?;
        let mut used_ids = BTreeSet::new();
        let mut operations = BTreeMap::new();
        let mut operation_aliases = BTreeMap::new();
        for (target, group) in ordered {
            check(cancelled)?;
            let mut id = group.preferred.as_str();
            if !used_ids.insert(id) {
                id = &group.fallback;
                anyhow::ensure!(used_ids.insert(id), "operation identity collision");
            }
            let Some(prepared) = &group.operation else {
                continue;
            };
            if prepared.id != id {
                self.association_dirty.insert(target.clone());
            }
            report.issues.extend(group.issues.iter().cloned());
            let operation = if prepared.id == id {
                Arc::clone(prepared)
            } else {
                let mut operation = prepared.as_ref().clone();
                operation.id = id.to_owned();
                Arc::new(operation)
            };
            let retained = self
                .operations
                .get(id)
                .filter(|old| **old == operation)
                .cloned();
            let row = retained.unwrap_or_else(|| {
                self.dirty_operations.insert(id.to_owned());
                operation
            });
            // The append cache and canonical map share the retained allocation,
            // including unchanged components inside a rewritten target.
            group.operation = Some(Arc::clone(&row));
            operations.insert(id.to_owned(), row);
            operation_aliases.insert(id.to_owned(), Arc::clone(&group.aliases));
        }
        for id in self.operations.keys() {
            check(cancelled)?;
            if !operations.contains_key(id) {
                self.retired_operations.insert(id.clone());
            }
        }
        self.dirty_operations
            .retain(|id| operations.contains_key(id));
        self.operations = operations;
        self.operation_aliases = Arc::new(operation_aliases);
        self.operations_pending = false;
        Ok(())
    }
}
