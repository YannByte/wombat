//! Safe event navigation and bounded original text access, without a second index.
use crate::{
    conversation_dto::*,
    dto::operation_error,
    session_events::{Event, MessageOrigin, Payload},
    usage_store::{EventReadBudget, EventTarget, Snapshot},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const MAX_OUTPUT: usize = 1024 * 1024;
const EVENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_EVENTS: usize = 2000;
const MAX_CURSOR: usize = 32768;
mod source;
#[cfg(test)]
mod tests;
use source::Reader;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScanCursor {
    binding: String,
    partition: usize,
    offset: usize,
    chunk: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FragmentCursor {
    binding: String,
    chunk: usize,
    offset: usize,
    separator: bool,
}
fn error(code: &'static str) -> anyhow::Error {
    operation_error(code, code)
}
fn check(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(error("CANCELLED"))
    } else {
        Ok(())
    }
}
fn binding(request: &Request) -> Result<String> {
    Ok(crate::hash(serde_json::to_vec(&(
        1,
        request.action,
        &request.snapshot_id,
        &request.selection,
        &request.query,
        &request.baseline_snapshot_id,
    ))?))
}
pub(crate) fn validate(request: &Request) -> Result<()> {
    if request.anchor.is_some()
        && (request.action != Action::Locate || request.cursor.is_some() || request.limit.is_some())
        || request.radius.is_some() && request.anchor.is_none()
        || request.radius.is_some_and(|n| n > 40)
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    let strings = request
        .selection
        .sources
        .iter()
        .chain(&request.selection.projects)
        .chain(&request.selection.threads)
        .chain(&request.selection.turns)
        .chain(&request.roots);
    if request.snapshot_id.is_empty()
        || request.snapshot_id.len() > 4096
        || strings.clone().any(|s| s.is_empty() || s.len() > 4096)
        || strings.count() > 400
        || request.limit.is_some_and(|n| n == 0 || n > 200)
        || request
            .cursor
            .as_ref()
            .is_some_and(|s| s.len() > MAX_CURSOR)
        || request
            .max_scan_bytes
            .is_some_and(|n| n == 0 || n > 16 * 1024 * 1024)
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    if request.selection.origins.len() > 10
        || request.selection.phases.len() > 3
        || request.selection.record_kinds.len() > 5
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    let scope = crate::usage_app_dto::Scope {
        timezone: request.selection.timezone.clone(),
        ..Default::default()
    };
    crate::usage_app::scope::timezone(&scope)?;
    for date in [&request.selection.since, &request.selection.until]
        .into_iter()
        .flatten()
    {
        crate::usage_app::scope::date(date)?;
    }
    if request
        .selection
        .since
        .as_ref()
        .zip(request.selection.until.as_ref())
        .is_some_and(|(a, b)| a >= b)
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    if (request.action == Action::Changes) != request.baseline_snapshot_id.is_some()
        || request
            .baseline_snapshot_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 4096)
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    if (request.action == Action::Search) != request.query.is_some()
        || request
            .query
            .as_ref()
            .is_some_and(|q| q.is_empty() || q.len() > 4096)
        || request.action == Action::Search
            && request.selection.threads.is_empty()
            && request.selection.projects.is_empty()
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    if request.action == Action::Read {
        if request.targets.is_empty()
            || request.targets.len() > 20
            || request.cursor.is_some()
            || request.limit.is_some()
        {
            return Err(error("INVALID_ARGUMENT"));
        }
        for target in &request.targets {
            if target.max_bytes.is_some_and(|n| n == 0 || n > 256 * 1024)
                || target.before > 20
                || target.after > 20
                || target.cursor.as_ref().is_some_and(|s| s.len() > MAX_CURSOR)
            {
                return Err(error("INVALID_ARGUMENT"));
            }
        }
    } else if !request.targets.is_empty() {
        return Err(error("INVALID_ARGUMENT"));
    }
    Ok(())
}
fn event_target(evidence: &Evidence) -> EventTarget {
    EventTarget {
        thread_id: evidence.thread_id.clone(),
        turn_id: evidence.turn_id.clone(),
    }
}
fn event(snapshot: &Snapshot, evidence: &Evidence, cancelled: &AtomicBool) -> Result<Arc<Event>> {
    if evidence.event_id.len() != 64
        || evidence.source_instance_id.is_empty()
        || evidence.turn_id.is_some() && evidence.thread_id.is_none()
    {
        return Err(error("INVALID_ARGUMENT"));
    }
    let target = event_target(evidence);
    let cursor = snapshot.event_cursor_at_offset(&target, evidence.event_offset, cancelled)?;
    let page = snapshot.event_page(
        &target,
        1,
        cursor.as_ref(),
        EventReadBudget {
            max_facts: 2000,
            max_bytes: EVENT_BYTES,
        },
        cancelled,
    )?;
    page.events
        .into_iter()
        .next()
        .filter(|e| {
            e.id() == evidence.event_id
                && e.position().source_instance_id.as_ref() == evidence.source_instance_id
        })
        .ok_or_else(|| error("NOT_FOUND"))
}
fn selected(
    event: &Event,
    selection: &Selection,
    projects: &BTreeMap<&str, &crate::usage_store::ThreadEntry>,
) -> Result<bool> {
    let has = |values: &[String], value: Option<&str>| {
        values.is_empty() || value.is_some_and(|v| values.iter().any(|x| x == v))
    };
    if !has(
        &selection.sources,
        Some(&event.position().source_instance_id),
    ) || !has(&selection.threads, event.thread_id())
        || !has(&selection.turns, event.turn_id())
    {
        return Ok(false);
    }
    let project = event
        .thread_id()
        .and_then(|id| projects.get(id))
        .and_then(|t| t.thread.project.as_deref());
    if !has(&selection.projects, project) {
        return Ok(false);
    }
    if selection.since.is_some() || selection.until.is_some() {
        let tz = crate::usage_app::scope::timezone(&crate::usage_app_dto::Scope {
            timezone: selection.timezone.clone(),
            ..Default::default()
        })?;
        let date = event
            .time()
            .timestamp
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&tz).date_naive().to_string());
        if let Some(date) = date {
            if selection.since.as_ref().is_some_and(|s| date < *s)
                || selection.until.as_ref().is_some_and(|s| date >= *s)
            {
                return Ok(false);
            }
        } else if !selection.include_undated {
            return Ok(false);
        }
    }
    if let Payload::Message {
        origin,
        content_phase,
        record_kind,
        ..
    } = event.payload()
    {
        let origins = &selection.origins;
        if if origins.is_empty() {
            !matches!(
                origin,
                MessageOrigin::AssistantVisible
                    | MessageOrigin::UserInput
                    | MessageOrigin::UserUnclassified
            )
        } else {
            !origins.contains(origin)
        } {
            return Ok(false);
        }
        if !selection.record_kinds.is_empty() && !selection.record_kinds.contains(record_kind) {
            return Ok(false);
        }
        if !selection.phases.is_empty() && !selection.phases.contains(content_phase) {
            return Ok(false);
        }
    } else if !selection.origins.is_empty()
        || !selection.phases.is_empty()
        || !selection.record_kinds.is_empty()
    {
        return Ok(false);
    }
    Ok(true)
}
fn activity(
    event: &Event,
    offset: usize,
    projects: &BTreeMap<&str, &crate::usage_store::ThreadEntry>,
    change: Option<Change>,
) -> Activity {
    let thread = event.thread_id().and_then(|id| projects.get(id));
    let (kind, origin, phase, native_id, name, status) = match event.payload() {
        Payload::Message {
            origin,
            content_phase,
            native_id,
            ..
        } => (
            "message",
            Some(*origin),
            Some(*content_phase),
            native_id.clone(),
            None,
            None,
        ),
        Payload::Operation { value, .. } => (
            "operation",
            None,
            None,
            None,
            Some(value.name.to_string()),
            Some(value.status.to_string()),
        ),
        Payload::Lifecycle { phase, .. } => (
            "lifecycle",
            None,
            None,
            None,
            None,
            Some(format!("{phase:?}").to_lowercase()),
        ),
        _ => ("other", None, None, None, None, None),
    };
    Activity {
        evidence: Evidence {
            event_id: event.id().into(),
            source_instance_id: event.position().source_instance_id.to_string(),
            thread_id: event.thread_id().map(Into::into),
            turn_id: event.turn_id().map(Into::into),
            event_offset: offset,
        },
        timestamp: event.time().timestamp.as_deref().map(Into::into),
        collected_at: event.collected_at().into(),
        project: thread.and_then(|t| t.thread.project.clone()),
        title: thread.and_then(|t| t.thread.title.clone()),
        kind: kind.into(),
        origin,
        phase,
        native_message_id: native_id,
        record_kind: match event.payload() {
            Payload::Message { record_kind, .. } => Some(*record_kind),
            _ => None,
        },
        name,
        operation_id: match event.payload() {
            Payload::Operation { value, .. } => Some(value.id.to_string()),
            _ => None,
        },
        status,
        has_text_locator: event
            .original_content()
            .is_some_and(|c| !c.chunks.is_empty()),
        content_partial: event.original_content().is_none_or(|c| c.partial),
        change,
    }
}
fn fragment(
    snapshot: &Snapshot,
    request: &Request,
    target: &ReadTarget,
    projects: &BTreeMap<&str, &crate::usage_store::ThreadEntry>,
    reader: &mut Reader<'_>,
    cancelled: &AtomicBool,
) -> Result<Fragment> {
    let event = event(snapshot, &target.evidence, cancelled)?;
    if !selected(&event, &request.selection, projects)? {
        return Err(error("NOT_FOUND"));
    }
    let content = event
        .original_content()
        .ok_or_else(|| error("CONTENT_UNAVAILABLE"))?;
    if content.chunks.is_empty() {
        return Err(error("CONTENT_UNAVAILABLE"));
    }
    let key = crate::hash(serde_json::to_vec(&(
        &request.snapshot_id,
        &request.selection,
        &target.evidence,
    ))?);
    let mut state = if let Some(cursor) = &target.cursor {
        serde_json::from_str::<FragmentCursor>(cursor).map_err(|_| error("INVALID_ARGUMENT"))?
    } else {
        FragmentCursor {
            binding: key.clone(),
            chunk: 0,
            offset: 0,
            separator: false,
        }
    };
    if state.binding != key
        || state.chunk > content.chunks.len()
        || state.chunk == content.chunks.len() && state.offset != 0
    {
        return Err(error("CURSOR_MISMATCH"));
    }
    let max = target.max_bytes.unwrap_or(8192);
    let mut text = String::new();
    let mut locations = vec![];
    while state.chunk < content.chunks.len() {
        check(cancelled)?;
        let chunk = &content.chunks[state.chunk];
        if !text.is_empty()
            && max.saturating_sub(text.len()) < chunk.bytes as usize + usize::from(state.separator)
        {
            break;
        }
        let (value, location) = reader.chunk(&event, chunk, cancelled)?;
        if state.offset > value.len() || !value.is_char_boundary(state.offset) {
            return Err(error("INVALID_ARGUMENT"));
        }
        if state.separator {
            if text.len() == max {
                break;
            }
            text.push('\n');
            state.separator = false;
        }
        let remaining = max.saturating_sub(text.len());
        let mut end = value.len().min(state.offset + remaining);
        while end > state.offset && !value.is_char_boundary(end) {
            end -= 1;
        }
        if end == state.offset && state.offset < value.len() {
            if text.is_empty() {
                return Err(error("RESOURCE_LIMIT"));
            }
            break;
        }
        text.push_str(&value[state.offset..end]);
        locations.push(location);
        state.offset = end;
        if end == value.len() {
            state.chunk += 1;
            state.offset = 0;
            state.separator = content
                .chunks
                .get(state.chunk)
                .is_some_and(|next| next.part != chunk.part);
        }
        if text.len() >= max {
            break;
        }
    }
    let next_cursor = (state.chunk < content.chunks.len())
        .then(|| serde_json::to_string(&state))
        .transpose()?;
    let mut context = vec![];
    let start = target.evidence.event_offset.saturating_sub(target.before);
    let end = target.evidence.event_offset.saturating_add(target.after);
    if target.before + target.after > 0 {
        let t = event_target(&target.evidence);
        let cursor = snapshot.event_cursor_at_offset(&t, start, cancelled)?;
        let page = snapshot.event_page(
            &t,
            end - start + 1,
            cursor.as_ref(),
            EventReadBudget {
                max_facts: 2000,
                max_bytes: EVENT_BYTES,
            },
            cancelled,
        )?;
        for (index, event) in page.events.iter().enumerate() {
            let offset = start + index;
            if offset != target.evidence.event_offset
                && selected(event, &request.selection, projects)?
            {
                context.push(activity(event, offset, projects, None));
            }
        }
    }
    Ok(Fragment {
        evidence: target.evidence.clone(),
        text,
        next_cursor,
        locations,
        context,
        partial: content.partial || !event.gaps().is_empty(),
    })
}

pub(crate) fn query(
    snapshot: &Snapshot,
    baseline: Option<&Snapshot>,
    request: &Request,
    cancelled: &AtomicBool,
) -> Result<Response> {
    validate(request)?;
    check(cancelled)?;
    if request.snapshot_id != snapshot.manifest.snapshot_ref.snapshot_id {
        return Err(error("VIEW_EXPIRED"));
    }
    let mut projects = BTreeMap::new();
    for thread in &snapshot.manifest.threads {
        if (!request.selection.threads.is_empty()
            && !request.selection.threads.contains(&thread.thread.id))
            || (!request.selection.projects.is_empty()
                && thread
                    .thread
                    .project
                    .as_ref()
                    .is_none_or(|p| !request.selection.projects.contains(p)))
            || (!request.selection.sources.is_empty()
                && !request
                    .selection
                    .sources
                    .contains(&thread.thread.source_instance_id))
        {
            continue;
        }
        if projects.len() >= 65536 {
            return Err(error("RESOURCE_LIMIT"));
        }
        projects.insert(thread.thread.id.as_str(), thread.as_ref());
    }
    let mut reader = Reader::new(snapshot, request.max_scan_bytes.unwrap_or(8 * 1024 * 1024));
    let mut response = Response {
        output_version: 1,
        action: request.action,
        snapshot_id: request.snapshot_id.clone(),
        baseline_snapshot_id: request.baseline_snapshot_id.clone(),
        selection: request.selection.clone(),
        items: vec![],
        fragments: vec![],
        next_cursor: None,
        scan_complete: true,
        partial: snapshot.manifest.sources.iter().any(|s| {
            (request.selection.sources.is_empty()
                || request.selection.sources.contains(&s.source.id))
                && s.status != "complete"
        }),
        limits: vec![],
        scanned_events: 0,
        source_bytes_read: 0,
        freshness: None,
        window: None,
    };
    if let Some(anchor) = &request.anchor {
        let anchor_event = event(snapshot, anchor, cancelled)?;
        let mut owner = request.selection.clone();
        owner.origins.clear();
        owner.phases.clear();
        owner.record_kinds.clear();
        if !selected(&anchor_event, &owner, &projects)? {
            return Err(error("NOT_FOUND"));
        }
        let radius = request.radius.unwrap_or(20);
        let start = anchor.event_offset.saturating_sub(radius);
        let target = event_target(anchor);
        let cursor = snapshot.event_cursor_at_offset(&target, start, cancelled)?;
        let page = snapshot.event_page(
            &target,
            anchor.event_offset - start + radius + 1,
            cursor.as_ref(),
            EventReadBudget {
                max_facts: 2000,
                max_bytes: EVENT_BYTES,
            },
            cancelled,
        )?;
        response.scanned_events = page.events.len();
        for (index, e) in page.events.iter().enumerate() {
            if matches!(e.payload(), Payload::Message { .. })
                && selected(e, &request.selection, &projects)?
            {
                response
                    .items
                    .push(activity(e, start + index, &projects, None));
            }
        }
        response.window = Some(ContextWindow {
            anchor: anchor.clone(),
            radius,
        });
    } else if request.action == Action::Read {
        for target in &request.targets {
            response.fragments.push(fragment(
                snapshot,
                request,
                target,
                &projects,
                &mut reader,
                cancelled,
            )?);
        }
    } else {
        let key = binding(request)?;
        let mut state = request
            .cursor
            .as_ref()
            .map(|s| serde_json::from_str::<ScanCursor>(s).map_err(|_| error("INVALID_ARGUMENT")))
            .transpose()?
            .unwrap_or(ScanCursor {
                binding: key.clone(),
                ..Default::default()
            });
        if state.binding != key
            || state.partition > snapshot.manifest.events.partitions.len()
            || (request.cursor.is_some()
                && (state.partition >= snapshot.manifest.events.partitions.len()
                    || state.offset >= snapshot.manifest.events.partitions[state.partition].count))
        {
            return Err(error("CURSOR_MISMATCH"));
        }
        let old = if request.action == Action::Changes {
            let old = baseline.ok_or_else(|| error("VIEW_EXPIRED"))?;
            if request.baseline_snapshot_id.as_deref()
                != Some(old.manifest.snapshot_ref.snapshot_id.as_str())
            {
                return Err(error("VIEW_EXPIRED"));
            }
            let roots = |s: &Snapshot| {
                s.manifest
                    .sources
                    .iter()
                    .map(|s| (s.source.id.clone(), s.source.root.clone()))
                    .collect::<BTreeMap<_, _>>()
            };
            if roots(old) != roots(snapshot) {
                return Err(error("INVALID_ARGUMENT"));
            }
            Some(
                old.manifest
                    .watermarks
                    .iter()
                    .map(|w| ((w.source_instance_id.as_str(), w.file_id.as_str()), w))
                    .collect::<BTreeMap<_, _>>(),
            )
        } else {
            None
        };
        let started = Instant::now();
        let limit = request.limit.unwrap_or(20);
        let mut metadata = 0;
        'scan: while state.partition < snapshot.manifest.events.partitions.len() {
            check(cancelled)?;
            if response.scanned_events >= MAX_EVENTS
                || metadata >= 10000
                || started.elapsed().as_millis() >= 80
            {
                response.limits.push("scan_budget".into());
                break;
            }
            let partition = &snapshot.manifest.events.partitions[state.partition];
            metadata += 1;
            if matches!(request.action, Action::Locate | Action::Search)
                && partition.native_boundary.message_count == 0
            {
                state.partition += 1;
                state.offset = 0;
                state.chunk = 0;
                continue;
            }
            let t = &partition.target;
            if !request.selection.threads.is_empty()
                && t.thread_id
                    .as_ref()
                    .is_none_or(|id| !request.selection.threads.contains(id))
                || !request.selection.turns.is_empty()
                    && t.turn_id
                        .as_ref()
                        .is_none_or(|id| !request.selection.turns.contains(id))
                || !request.selection.projects.is_empty()
                    && t.thread_id
                        .as_deref()
                        .and_then(|id| projects.get(id))
                        .and_then(|t| t.thread.project.as_ref())
                        .is_none_or(|p| !request.selection.projects.contains(p))
            {
                state.partition += 1;
                state.offset = 0;
                state.chunk = 0;
                continue;
            }
            if state.offset >= partition.count {
                state.partition += 1;
                state.offset = 0;
                state.chunk = 0;
                continue;
            }
            let cursor = snapshot.event_cursor_at_offset(t, state.offset, cancelled)?;
            let page = snapshot.event_page(
                t,
                64,
                cursor.as_ref(),
                EventReadBudget {
                    max_facts: 2000,
                    max_bytes: EVENT_BYTES,
                },
                cancelled,
            )?;
            let page_start = state.offset;
            for (index, e) in page.events.iter().enumerate() {
                state.offset = page_start + index;
                if response.scanned_events >= MAX_EVENTS || started.elapsed().as_millis() >= 80 {
                    response.limits.push("scan_budget".into());
                    break 'scan;
                }
                response.scanned_events += 1;
                let offset = state.offset;
                let is_message = matches!(e.payload(), Payload::Message { .. });
                let change = old.as_ref().and_then(|old| {
                    let p = e.position();
                    match old.get(&(p.source_instance_id.as_ref(), p.file_id.as_ref())) {
                        None => Some(Change::NewSourceFile),
                        Some(w) if w.generation.as_deref() != Some(p.generation.as_ref()) => {
                            Some(Change::GenerationChanged)
                        }
                        Some(w) if p.byte_offset >= w.committed_offset => Some(Change::Appended),
                        _ => None,
                    }
                });
                if selected(e, &request.selection, &projects)?
                    && (!matches!(request.action, Action::Locate | Action::Search) || is_message)
                    && (request.action != Action::Changes || change.is_some())
                {
                    if request.action == Action::Search {
                        if let Some(content) = e.original_content() {
                            let needle = request.query.as_deref().unwrap();
                            let before_chunk = state.chunk;
                            match source::search(
                                &mut reader,
                                e,
                                content,
                                needle,
                                &mut state.chunk,
                                cancelled,
                            ) {
                                Ok(true) => {
                                    response.items.push(activity(e, offset, &projects, change))
                                }
                                Ok(false) => {}
                                Err(err)
                                    if err
                                        .downcast_ref::<crate::dto::OperationError>()
                                        .is_some_and(|e| e.code == "RESOURCE_LIMIT") =>
                                {
                                    if state.chunk == before_chunk && response.items.is_empty() {
                                        return Err(err);
                                    }
                                    response.limits.push("source_read_budget".into());
                                    break 'scan;
                                }
                                Err(err) => return Err(err),
                            }
                            response.partial |= content.partial;
                        } else {
                            response.partial = true;
                            response.limits.push("content_unavailable".into());
                        }
                    } else {
                        response.items.push(activity(e, offset, &projects, change));
                    }
                }
                state.offset += 1;
                state.chunk = 0;
                if response.items.len() >= limit {
                    break 'scan;
                }
            }
        }
        while state.partition < snapshot.manifest.events.partitions.len()
            && state.offset >= snapshot.manifest.events.partitions[state.partition].count
        {
            state.partition += 1;
            state.offset = 0;
        }
        response.scan_complete = state.partition == snapshot.manifest.events.partitions.len();
        response.next_cursor = (!response.scan_complete)
            .then(|| serde_json::to_string(&state))
            .transpose()?;
    }
    response.partial |= response.fragments.iter().any(|f| f.partial) || !response.limits.is_empty();
    response.source_bytes_read = reader.bytes;
    response.limits.sort();
    response.limits.dedup();
    if serde_json::to_vec(&response)?.len() > MAX_OUTPUT {
        return Err(error("RESOURCE_LIMIT"));
    }
    Ok(response)
}
pub(crate) fn dispatch(request: Request) -> Result<Response> {
    let snapshot = crate::usage_store::load(Some(&request.snapshot_id))?;
    let baseline = request
        .baseline_snapshot_id
        .as_deref()
        .map(|id| crate::usage_store::load(Some(id)))
        .transpose()?;
    query(
        &snapshot,
        baseline.as_ref(),
        &request,
        &AtomicBool::new(false),
    )
}
