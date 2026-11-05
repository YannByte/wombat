//! Conversation presence uses event evidence, independent of metering and accounting.
use super::*;

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    binding: String,
    record: usize,
    thread_id: Option<String>,
    event_cursor: Option<String>,
    skip: usize,
}

pub(super) fn query(
    snapshot: &Snapshot,
    request: &Request,
    cancelled: &AtomicBool,
    mut response: Response,
    scan_millis: u128,
) -> Result<Response> {
    let key = crate::hash(serde_json::to_vec(&(binding(request)?, request.offset))?);
    let mut cursor = if let Some(cursor) = &request.cursor {
        let value: Cursor = serde_json::from_str(cursor).map_err(|_| cursor_error("/cursor"))?;
        if value.binding != key {
            return Err(error("INVALID_ARGUMENT"));
        }
        value
    } else {
        Cursor {
            binding: key,
            skip: request.offset.unwrap_or(0),
            ..Default::default()
        }
    };
    let needle = request.query.as_deref().unwrap_or("").to_lowercase();
    let mut threads = snapshot
        .manifest
        .threads
        .iter()
        .filter(|entry| {
            let t = &entry.thread;
            (request.selection.threads.is_empty() || request.selection.threads.contains(&t.id))
                && (request.selection.projects.is_empty()
                    || t.project
                        .as_ref()
                        .is_some_and(|p| request.selection.projects.contains(p)))
                && (request.selection.sources.is_empty()
                    || request.selection.sources.contains(&t.source_instance_id))
                && (needle.is_empty()
                    || t.id.to_lowercase().contains(&needle)
                    || t.upstream_id.to_lowercase().contains(&needle)
                    || t.title
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&needle))
        })
        .collect::<Vec<_>>();
    if threads.len() > 65536 {
        return Err(error("RESOURCE_LIMIT"));
    }
    threads.sort_by(|a, b| {
        b.thread
            .last_activity_at
            .cmp(&a.thread.last_activity_at)
            .then(a.thread.id.cmp(&b.thread.id))
    });
    if request.cursor.is_some() {
        let id = cursor
            .thread_id
            .as_deref()
            .ok_or_else(|| error("CURSOR_MISMATCH"))?;
        cursor.record = threads
            .iter()
            .position(|t| t.thread.id == id)
            .ok_or_else(|| error("SOURCE_CHANGED"))?;
    }
    if cursor.record > threads.len() {
        return Err(error("INVALID_ARGUMENT"));
    }
    let started = Instant::now();
    let limit = request.limit.unwrap_or(20);
    while cursor.record < threads.len() {
        check(cancelled)?;
        if response.scanned_events >= MAX_EVENTS
            || response.scanned_events > 0 && started.elapsed().as_millis() >= scan_millis
        {
            response.limits.push("scan_budget".into());
            break;
        }
        let mut inner = request.clone();
        inner.action = Action::Activity;
        inner.selection.threads = vec![threads[cursor.record].thread.id.clone()];
        inner.query = None;
        inner.limit = Some(1);
        inner.offset = None;
        inner.cursor = cursor.event_cursor.take();
        let result = query_with_scan_budget(snapshot, None, &inner, cancelled, scan_millis)?;
        response.scanned_events += result.scanned_events;
        if let Some(mut item) = result.items.into_iter().next() {
            let thread = &threads[cursor.record].thread;
            item.record = Some(RecordSummary {
                upstream_id: thread.upstream_id.clone(),
                started_at: thread.started_at.clone(),
                last_activity_at: thread.last_activity_at.clone(),
            });
            if cursor.skip > 0 {
                cursor.skip -= 1;
            } else {
                response.items.push(item);
            }
            cursor.record += 1;
        } else if result.scan_complete {
            cursor.record += 1;
        } else {
            cursor.event_cursor = result.next_cursor;
            response.limits.extend(result.limits);
            break;
        }
        if response.items.len() == limit {
            break;
        }
    }
    response.scan_complete = cursor.record == threads.len();
    response.partial |= !response.scan_complete;
    response.next_cursor = if response.scan_complete {
        None
    } else {
        cursor.thread_id = Some(threads[cursor.record].thread.id.clone());
        Some(serde_json::to_string(&cursor)?)
    };
    if response
        .next_cursor
        .as_ref()
        .is_some_and(|s| s.len() > MAX_CURSOR)
        || serde_json::to_vec(&response)?.len() > MAX_OUTPUT
    {
        return Err(error("OUTPUT_LIMIT"));
    }
    Ok(response)
}
