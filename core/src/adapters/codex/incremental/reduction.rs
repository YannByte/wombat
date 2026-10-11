//! Ordinary restart orders accounting observations on disk. Association graphs
//! are local to one target; only canonical components survive into append cache.
use super::*;

pub(super) fn load_streamed(
    db: &Connection,
    scope: &str,
    report: &SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Facts> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS codex_replay(id TEXT PRIMARY KEY,file TEXT NOT NULL,offset BLOB NOT NULL,ordinal INTEGER NOT NULL,payload TEXT NOT NULL); DELETE FROM codex_replay;")?;
    let mut insert =
        db.prepare_cached("INSERT OR REPLACE INTO codex_replay VALUES(?1,?2,?3,?4,?5)")?;
    let mut facts = Facts::default();
    let mut event_strings = crate::session_events::EventStrings::default();
    let mut replay_report = report.clone();
    replay_report.issues.clear();
    let mut groups = BTreeMap::new();
    crate::live_index::each_event_group(
        db,
        scope,
        None,
        false,
        |event| {
            crate::operation_association::check(cancelled)?;
            event_strings.share(Arc::make_mut(event));
            anyhow::ensure!(
                event.position().source_instance_id.as_ref() == report.source.id,
                "stored event source identity mismatch"
            );
            if let Some(input) = event_projection::Replay::from_event(event) {
                let (file, offset, ordinal) = event_projection::order(event);
                insert.execute(rusqlite::params![
                    event.id(),
                    file,
                    offset.to_be_bytes().as_slice(),
                    ordinal,
                    serde_json::to_string(&input)?
                ])?;
            }
            Ok(event_projection::association_input(event))
        },
        |target, events| {
            groups.insert(
                target,
                Facts::prepare_associations(
                    events.iter().map(AsRef::as_ref),
                    &mut facts.strings,
                    &mut replay_report,
                    cancelled,
                )?,
            );
            Ok(())
        },
    )?;
    let mut query =
        db.prepare_cached("SELECT payload FROM codex_replay ORDER BY file,offset,ordinal,id")?;
    let mut cursor = query.query([])?;
    while let Some(row) = cursor.next()? {
        crate::operation_association::check(cancelled)?;
        let input: event_projection::Replay<'_> = serde_json::from_str(row.get_ref(0)?.as_str()?)?;
        input.apply(&mut facts, &mut replay_report);
    }
    drop(cursor);
    db.execute("DELETE FROM codex_replay", [])?;
    facts.association_groups = Some(groups);
    facts.publish_associations(&mut replay_report, cancelled)?;
    facts.streaming = true;
    Ok(facts)
}

pub(super) fn resolve_streamed(
    db: &Connection,
    scope: &str,
    facts: &mut Facts,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    if !facts.operations_pending {
        return Ok(());
    }
    let changed = facts
        .association_groups
        .as_ref()
        .map(|_| crate::live_index::changed_event_targets(db))
        .transpose()?;
    let mut groups = facts.association_groups.take().unwrap_or_default();
    if let Some(changed) = &changed {
        for target in changed {
            groups.remove(target);
        }
    }
    let mut strings = crate::session_events::EventStrings::default();
    let source_id = report.source.id.clone();
    crate::live_index::each_event_group(
        db,
        scope,
        changed.as_deref(),
        true,
        |event| {
            crate::operation_association::check(cancelled)?;
            anyhow::ensure!(
                event.position().source_instance_id.as_ref() == source_id,
                "stored event source identity mismatch"
            );
            strings.share(Arc::make_mut(event));
            Ok(event_projection::association_input(event))
        },
        |target, events| {
            groups.insert(
                target,
                Facts::prepare_associations(
                    events.iter().map(AsRef::as_ref),
                    &mut facts.strings,
                    report,
                    cancelled,
                )?,
            );
            Ok(())
        },
    )?;
    facts.association_groups = Some(groups);
    facts.publish_associations(report, cancelled)
}
