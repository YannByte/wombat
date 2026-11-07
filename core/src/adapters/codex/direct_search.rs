//! Read-only visible text search. It owns no product storage or ledger.
mod context;
use super::*;
use crate::{conversation_dto::*, dto::operation_error, file_metadata::FileMetadata};
use anyhow::Result;
use memchr::memmem::Finder;
use std::{
    borrow::Cow,
    io::BufRead,
    time::{Instant, UNIX_EPOCH},
};

const MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_LINE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 20_000;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_SCAN_MILLIS: u128 = 5_000;
const USER: crate::session_events::MessageOrigin =
    crate::session_events::MessageOrigin::UserUnclassified;
const ASSISTANT: crate::session_events::MessageOrigin =
    crate::session_events::MessageOrigin::AssistantVisible;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    binding: String,
    file: usize,
    offset: u64,
}

struct SourceFile {
    source: String,
    root: String,
    path: PathBuf,
    meta: FileMetadata,
}

#[derive(Default, Deserialize)]
struct Header<'a> {
    #[serde(borrow)]
    id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    session_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    cwd: Option<Cow<'a, str>>,
}

#[derive(Deserialize)]
struct LineMessage<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    role: Option<Cow<'a, str>>,
    #[serde(borrow)]
    thread_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    item_id: Option<Cow<'a, str>>,
    #[serde(borrow)]
    message: Option<&'a serde_json::value::RawValue>,
    #[serde(borrow)]
    content: Option<&'a serde_json::value::RawValue>,
    #[serde(borrow)]
    item: Option<&'a serde_json::value::RawValue>,
    internal_chat_message_metadata_passthrough: Option<MessageMetadata>,
}

#[derive(Deserialize)]
struct MessageMetadata {
    content_item_kinds: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Part<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    text: Option<&'a serde_json::value::RawValue>,
}

#[derive(Deserialize)]
struct OriginMetadata {
    #[serde(default)]
    inherited_user_message: bool,
}

fn err(code: &'static str, message: &'static str) -> anyhow::Error {
    operation_error(code, message)
}

fn inventory(request: &Request) -> (Vec<SourceFile>, bool) {
    let discovered = CodexAdapter.discover(&DiscoveryRequest {
        roots: request.roots.iter().map(PathBuf::from).collect(),
    });
    let mut partial = !discovered.issues.is_empty();
    let mut files = Vec::new();
    for source in discovered.sources {
        if !request.selection.sources.is_empty() && !request.selection.sources.contains(&source.id)
        {
            continue;
        }
        let root = Path::new(&source.root);
        let context = RunContext {
            max_files: MAX_FILES.saturating_sub(files.len()),
            ..Default::default()
        };
        let mut report = SourceReport {
            source: source.clone(),
            adapter_version: VERSION.into(),
            source_versions: vec![],
            capabilities: CodexAdapter.descriptor().capabilities,
            status: "complete".into(),
            files_read: 0,
            bytes_read: 0,
            issues: vec![],
        };
        let mut paths = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for sub in ["sessions", "archived_sessions"] {
            list_files(
                &root.join(sub),
                0,
                &mut paths,
                &mut visited,
                &context,
                &mut report,
            );
        }
        partial |= !report.issues.is_empty();
        for path in paths {
            match FileMetadata::read(&path) {
                Ok(meta) if meta.is_file() => files.push(SourceFile {
                    source: source.id.clone(),
                    root: source.root.clone(),
                    path,
                    meta,
                }),
                _ => partial = true,
            }
        }
    }
    (files, partial)
}

struct Message<'a> {
    origin: crate::session_events::MessageOrigin,
    native_id: Option<Cow<'a, str>>,
    parts: Vec<TextPart<'a>>,
    partial: bool,
}

struct TextPart<'a> {
    text: Cow<'a, str>,
    encoded_bytes: usize,
}

fn text_part(raw: &serde_json::value::RawValue) -> Option<TextPart<'_>> {
    let encoded = raw.get();
    let inner = encoded.strip_prefix('"')?.strip_suffix('"')?;
    let text = if inner.as_bytes().contains(&b'\\') {
        Cow::Owned(serde_json::from_str(encoded).ok()?)
    } else {
        Cow::Borrowed(inner)
    };
    Some(TextPart {
        text,
        encoded_bytes: if inner.is_empty() { 2 } else { inner.len() },
    })
}

fn origin_and_text<'a>(envelope: &Envelope<'a>, upstream: &str) -> Result<Option<Message<'a>>> {
    if envelope.metadata.is_some_and(|m| {
        serde_json::from_str::<OriginMetadata>(m.get()).map_or(true, |m| m.inherited_user_message)
    }) {
        return Ok(None);
    }
    let row: LineMessage<'_> = serde_json::from_str(envelope.payload.get())?;
    if row.thread_id.as_deref().is_some_and(|id| id != upstream) {
        return Ok(None);
    }
    let native = envelope.kind == "event_msg"
        && matches!(row.kind.as_deref(), Some("item_completed" | "item_started"));
    let item = if native {
        let Some(item) = row.item else {
            return Ok(None);
        };
        serde_json::from_str(item.get())?
    } else {
        row
    };
    let origin = match (envelope.kind, item.kind.as_deref(), item.role.as_deref()) {
        ("response_item", Some("message"), Some("user")) => USER,
        ("response_item", Some("message"), Some("assistant")) => ASSISTANT,
        ("user_message", _, _) => USER,
        ("agent_message", _, _) => ASSISTANT,
        ("event_msg", Some("user_message" | "UserMessage" | "userMessage"), _) => USER,
        ("event_msg", Some("agent_message" | "AgentMessage" | "agentMessage"), _) => ASSISTANT,
        _ => return Ok(None),
    };
    if origin == USER
        && item
            .internal_chat_message_metadata_passthrough
            .and_then(|m| m.content_item_kinds)
            .is_some_and(|kinds| {
                !kinds.is_empty()
                    && kinds.iter().all(|kind| {
                        matches!(
                            kind.as_str(),
                            "agents_md.instructions"
                                | "environments.environment_context"
                                | "additional_content.codex_apps_open_page"
                        )
                    })
            })
    {
        return Ok(None);
    }
    let native_id = item.id.or(item.item_id).filter(|id| !id.is_empty());
    let mut partial = false;
    let mut parts = Vec::new();
    if let Some(raw) = item.message {
        parts.extend(text_part(raw));
        partial = parts.is_empty();
    } else if let Some(raw) = item.content {
        let values: Vec<&serde_json::value::RawValue> = serde_json::from_str(raw.get())?;
        if values.is_empty() {
            parts.push(TextPart {
                text: Cow::Borrowed(""),
                encoded_bytes: raw.get().len(),
            });
        }
        for value in values {
            let part = serde_json::from_str::<Part<'_>>(value.get())
                .ok()
                .filter(|p| {
                    if native && origin == ASSISTANT {
                        p.kind.as_deref() == Some("Text")
                    } else {
                        matches!(p.kind.as_deref(), Some("input_text" | "output_text"))
                    }
                })
                .and_then(|p| p.text)
                .and_then(text_part);
            if let Some(part) = part {
                parts.push(part);
            } else {
                partial = true;
            }
        }
    } else {
        return Ok(None);
    }
    Ok(Some(Message {
        origin,
        native_id,
        parts,
        partial,
    }))
}

// Codex writes compact envelope and item headers. Reject known unrelated rows
// without inspecting their potentially large body. Other key orders, whitespace
// and escaped header values go through the ordinary JSON decoder.
fn compact_candidate(line: &[u8], users: bool, assistants: bool) -> Option<bool> {
    fn field<'a>(bytes: &'a [u8], key: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
        let tail = bytes.strip_prefix(key)?;
        let end = memchr::memchr2(b'"', b'\\', tail.get(..128).unwrap_or(tail))?;
        (tail[end] == b'"').then(|| (&tail[..end], &tail[end + 1..]))
    }
    let (_, mut rest) = field(line, b"{\"timestamp\":\"")?;
    if let Some(ordinal) = rest.strip_prefix(b",\"ordinal\":") {
        let end = memchr::memchr(b',', ordinal.get(..32).unwrap_or(ordinal))?;
        if !ordinal[..end].iter().all(u8::is_ascii_digit) {
            return None;
        }
        rest = &ordinal[end..];
    }
    let (kind, rest) = field(rest, b",\"type\":\"")?;
    if kind == b"user_message" {
        return Some(users);
    }
    if kind == b"agent_message" {
        return Some(assistants);
    }
    if !matches!(kind, b"event_msg" | b"response_item") {
        return Some(false);
    }
    let (item, rest) = field(rest, b",\"payload\":{\"type\":\"")?;
    if kind == b"response_item" {
        if item != b"message" {
            return Some(false);
        }
        let (role, _) = field(rest, b",\"role\":\"").or_else(|| {
            let (_, rest) = field(rest, b",\"id\":\"")?;
            field(rest, b",\"role\":\"")
        })?;
        return Some(users && role == b"user" || assistants && role == b"assistant");
    }
    let item = if matches!(item, b"item_completed" | b"item_started") {
        let (_, rest) = field(rest, b",\"thread_id\":\"")?;
        let (_, rest) = field(rest, b",\"turn_id\":\"")?;
        field(rest, b",\"item\":{\"type\":\"")?.0
    } else {
        item
    };
    Some(
        users && matches!(item, b"user_message" | b"UserMessage" | b"userMessage")
            || assistants && matches!(item, b"agent_message" | b"AgentMessage" | b"agentMessage"),
    )
}

fn excerpt(text: &str, start: usize, length: usize, cap: usize) -> (String, MatchRange) {
    let before = ((cap - length) / 2).min(512);
    let mut left = start.saturating_sub(before);
    while !text.is_char_boundary(left) {
        left += 1;
    }
    let mut right = (left + cap).min(text.len());
    while !text.is_char_boundary(right) {
        right -= 1;
    }
    (
        text[left..right].into(),
        MatchRange {
            start: start - left,
            end: start - left + length,
        },
    )
}

pub(crate) fn query(request: &Request) -> Result<Response> {
    let s = &request.selection;
    let cap = request.excerpt_bytes.unwrap_or(8192);
    let limit = request.limit.unwrap_or(200);
    let terms: Vec<&str> = request
        .query
        .iter()
        .chain(request.terms.iter().flatten())
        .map(String::as_str)
        .collect();
    if terms.is_empty()
        || terms.len() > MAX_SEARCH_TERMS
        || terms
            .iter()
            .any(|t| t.trim().is_empty() || t.len() > MAX_TERM_BYTES || t.len() > cap)
        || request.query.is_some() && request.terms.is_some()
        || !(1..=65536).contains(&cap)
        || !(1..=200).contains(&limit)
        || s.include_tool_output
        || s.operation_id.is_some()
        || !s.turns.is_empty()
        || !s.phases.is_empty()
        || !s.record_kinds.is_empty()
        || s.origins.iter().any(|o| {
            !matches!(
                *o,
                USER | ASSISTANT | crate::session_events::MessageOrigin::UserInput
            )
        })
        || request.anchor.is_some()
        || request.radius.is_some()
        || request.offset.is_some()
        || request
            .context_messages
            .is_some_and(|n| !(1..=5).contains(&n))
        || !request.targets.is_empty()
        || request.max_scan_bytes.is_some()
        || request.cursor.as_ref().is_some_and(|v| v.len() > 4096)
        || request.roots.len() + s.projects.len() + s.sources.len() + s.threads.len() > 400
    {
        return Err(err(
            "INVALID_ARGUMENT",
            "Direct search requires literal terms and reads visible messages only.",
        ));
    }
    let date = |value: &Option<String>| {
        value
            .as_deref()
            .map(crate::usage_app::scope::date)
            .transpose()
    };
    let (since, until) = (date(&s.since)?, date(&s.until)?);
    if since.is_some() != until.is_some() || since.zip(until).is_some_and(|(a, b)| a >= b) {
        return Err(err(
            "INVALID_ARGUMENT",
            "Set both search dates and place the start before the exclusive end",
        ));
    }
    let zone = crate::usage_app::scope::timezone(&crate::usage_app_dto::Scope {
        timezone: s.timezone.clone(),
        ..Default::default()
    })?;
    let in_calendar = |time: Option<&str>| {
        let day = time
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&zone).date_naive());
        match day {
            Some(day) => since.is_none_or(|x| day >= x) && until.is_none_or(|x| day < x),
            None => since.is_none() && until.is_none() || s.include_undated,
        }
    };
    let (files, mut partial) = inventory(request);
    let mut scope = Sha256::new();
    scope.update(serde_json::to_vec(&(1, &request.roots, s))?);
    for f in &files {
        scope.update(serde_json::to_vec(&(
            &f.source,
            &f.path,
            &f.meta.physical,
            f.meta.len(),
            f.meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos()),
        ))?);
    }
    let scope = format!("{:x}", scope.finalize());
    if request
        .source_scope
        .as_ref()
        .is_some_and(|prior| prior != &scope)
    {
        return Err(err(
            "SOURCE_CHANGED",
            "Direct source range changed. Restart with the original filters and no cursor.",
        ));
    }
    let binding = crate::hash(serde_json::to_vec(&(&scope, &terms))?);
    let mut cursor = match &request.cursor {
        Some(value) => serde_json::from_str::<Cursor>(value)
            .map_err(|_| err("INVALID_ARGUMENT", "Malformed direct search cursor"))?,
        None => Cursor {
            binding: binding.clone(),
            file: 0,
            offset: 0,
        },
    };
    if cursor.binding != binding || cursor.file > files.len() {
        return Err(err(
            "SOURCE_CHANGED",
            "Direct search cursor no longer matches this source range",
        ));
    }
    let query: Vec<_> = terms
        .iter()
        .map(|term| Finder::new(term.as_bytes()))
        .collect();
    let users = s.origins.is_empty() || s.origins.contains(&USER);
    let assistants = s.origins.is_empty() || s.origins.contains(&ASSISTANT);
    let mut hits = Vec::new();
    let mut source_files = Vec::new();
    let mut context = request.context_messages.map(context::ReadContext::new);
    let mut source_bytes = 0;
    let mut inspected = 0;
    let mut rows = 0;
    let mut output =
        serde_json::to_vec(&(&request.roots, s, &request.query, &request.terms))?.len() + 8192;
    let mut limits = Vec::new();
    let started = Instant::now();
    let mut line = Vec::new();
    'files: while let Some(f) = files.get(cursor.file) {
        if source_bytes >= MAX_SOURCE_BYTES || started.elapsed().as_millis() >= MAX_SCAN_MILLIS {
            limits.push("source_scan_budget".into());
            break;
        }
        let path = dunce::canonicalize(&f.path)?;
        if !path.starts_with(Path::new(&f.root)) {
            return Err(err("SOURCE_CHANGED", "Log path left its source root"));
        }
        let file = File::open(&path)?;
        let observed = FileMetadata::from_file(&file)?;
        if observed.physical != f.meta.physical || observed.len() < f.meta.len() {
            return Err(err(
                "SOURCE_CHANGED",
                "Log was replaced or truncated during direct search",
            ));
        }
        let mut header_reader = BufReader::with_capacity(16 * 1024, file.take(f.meta.len()));
        line.clear();
        let header_n = header_reader
            .by_ref()
            .take(MAX_LINE_BYTES)
            .read_until(b'\n', &mut line)? as u64;
        source_bytes += header_n;
        let header = line
            .ends_with(b"\n")
            .then(|| serde_json::from_slice::<Envelope>(&line).ok())
            .flatten()
            .filter(|e| e.kind == "session_meta")
            .and_then(|e| serde_json::from_str::<Header>(e.payload.get()).ok());
        let Some((upstream, project)) =
            header.and_then(|h| h.id.or(h.session_id).map(|id| (id, h.cwd)))
        else {
            partial = true;
            cursor.file += 1;
            cursor.offset = 0;
            continue;
        };
        let upstream = upstream.into_owned();
        let project = project.map(Cow::into_owned);
        if !s.projects.is_empty() && !project.as_ref().is_some_and(|p| s.projects.contains(p)) {
            cursor.file += 1;
            cursor.offset = 0;
            continue;
        }
        let thread = thread_identity(&f.source, &upstream);
        if !s.threads.is_empty() && !s.threads.contains(&thread) {
            cursor.file += 1;
            cursor.offset = 0;
            continue;
        }
        let mut offset = cursor.offset.max(header_n);
        if offset > f.meta.len() {
            return Err(err(
                "SOURCE_CHANGED",
                "Direct cursor is outside the source file",
            ));
        }
        // Avoid filling a large body buffer for files excluded by project or chat.
        let mut file = header_reader.into_inner().into_inner();
        if offset > header_n {
            file.seek(SeekFrom::Start(offset - 1))?;
            let mut boundary = [0];
            file.read_exact(&mut boundary)?;
            if boundary[0] != b'\n' {
                return Err(err(
                    "INVALID_ARGUMENT",
                    "Direct search cursor must start at a JSONL record boundary",
                ));
            }
        } else {
            file.seek(SeekFrom::Start(offset))?;
        }
        let mut reader = BufReader::with_capacity(1024 * 1024, file.take(f.meta.len() - offset));
        let mut file_index = None;
        let first_hit = hits.len();
        while offset < f.meta.len() {
            cursor.offset = offset;
            rows += 1;
            if source_bytes >= MAX_SOURCE_BYTES
                || rows & 1023 == 1 && started.elapsed().as_millis() >= MAX_SCAN_MILLIS
            {
                limits.push("source_scan_budget".into());
                break;
            }
            line.clear();
            let n = reader
                .by_ref()
                .take(MAX_LINE_BYTES + 1)
                .read_until(b'\n', &mut line)? as u64;
            if n == 0 {
                break;
            }
            source_bytes += n;
            let at = offset;
            offset += n;
            if !line.ends_with(b"\n") {
                partial = true;
                if n > MAX_LINE_BYTES {
                    let tail = reader
                        .by_ref()
                        .take(MAX_SOURCE_BYTES.saturating_sub(source_bytes))
                        .skip_until(b'\n')? as u64;
                    source_bytes += tail;
                    offset += tail;
                    if source_bytes >= MAX_SOURCE_BYTES && offset < f.meta.len() {
                        return Err(err(
                            "RESOURCE_LIMIT",
                            "An oversized log record exhausted the source budget",
                        ));
                    }
                }
                continue;
            }
            if compact_candidate(&line, users, assistants) == Some(false) {
                continue;
            }
            let Ok(envelope) = serde_json::from_slice::<Envelope>(&line) else {
                partial = true;
                continue;
            };
            if !in_calendar(envelope.timestamp) {
                continue;
            }
            let Message {
                origin: speaker,
                parts,
                ..
            } = match origin_and_text(&envelope, &upstream) {
                Ok(Some(found)) => found,
                Ok(None) => continue,
                Err(_) => {
                    partial = true;
                    continue;
                }
            };
            if !s.origins.is_empty() && !s.origins.contains(&speaker) {
                continue;
            }
            inspected += 1;
            let Some((part, start, term)) = parts.iter().find_map(|part| {
                query
                    .iter()
                    .enumerate()
                    .filter_map(|(i, finder)| finder.find(part.text.as_bytes()).map(|pos| (pos, i)))
                    .min()
                    .map(|(start, index)| (part, start, terms[index]))
            }) else {
                continue;
            };
            let (text, match_range) = excerpt(&part.text, start, term.len(), cap);
            let new_file = file_index.is_none().then(|| SourceSearchFile {
                source_instance_id: f.source.clone(),
                thread_id: thread.clone(),
                upstream_id: upstream.clone(),
                project: project.clone(),
                file: path.to_string_lossy().into_owned(),
            });
            let hit = SourceHit {
                file_index: file_index.unwrap_or(source_files.len()),
                timestamp: envelope.timestamp.map(str::to_owned),
                origin: speaker,
                byte_start: at,
                byte_end: offset,
                sha256: crate::hash(&line),
                matched_term: term.into(),
                match_range,
                text,
                context: None,
            };
            let mut size = serde_json::to_vec(&hit)?.len() + 1;
            if let Some(file) = &new_file {
                size += serde_json::to_vec(file)?.len() + 1;
            }
            if output + size > MAX_OUTPUT_BYTES {
                if hits.is_empty() {
                    return Err(err(
                        "OUTPUT_LIMIT",
                        "Direct search excerpt exceeds the response budget",
                    ));
                }
                limits.push("output_budget".into());
                break;
            }
            output += size;
            if let Some(file) = new_file {
                file_index = Some(source_files.len());
                source_files.push(file);
            }
            hits.push(hit);
            cursor.offset = offset;
            if hits.len() >= limit {
                break;
            }
        }
        if let Some(context) = &mut context
            && hits.len() > first_hit
        {
            let mut file = reader.into_inner().into_inner();
            file.seek(SeekFrom::Start(header_n))?;
            context.attach(
                file.take(f.meta.len() - header_n),
                header_n,
                &mut hits[first_hit..],
                &upstream,
                &in_calendar,
                &mut output,
            )?;
        }
        let after = FileMetadata::read(&path)?;
        if after.physical != f.meta.physical
            || after.len() < f.meta.len()
            || after.len() == f.meta.len() && after.modified().ok() != f.meta.modified().ok()
        {
            return Err(err(
                "SOURCE_CHANGED",
                "Log changed during direct search. Restart with the same filters.",
            ));
        }
        if offset >= f.meta.len() && (hits.len() >= limit || limits.is_empty()) {
            cursor.file += 1;
            cursor.offset = 0;
        }
        if hits.len() >= limit || !limits.is_empty() {
            break 'files;
        }
    }
    let done = cursor.file == files.len();
    let next_cursor = (!done)
        .then(|| serde_json::to_string(&cursor))
        .transpose()?;
    let next_search = next_cursor.as_ref().map(|c| {
        let mut next = request.clone();
        next.cursor = Some(c.clone());
        next.source_scope = Some(scope.clone());
        next
    });
    let (source_neighbors, context_bytes, context_partial) = context
        .map(context::ReadContext::finish)
        .unwrap_or_default();
    if context_partial {
        limits.push("context_budget".into());
    }
    Ok(Response {
        record: None,
        output_version: OUTPUT_VERSION,
        action: Action::SourceSearch,
        snapshot_id: format!("source:{scope}"),
        baseline_snapshot_id: None,
        selection: s.clone(),
        roots: request.roots.clone(),
        items: vec![],
        fragments: vec![],
        neighbors: vec![],
        source_hits: hits,
        source_files,
        source_neighbors,
        next_search,
        read_issues: vec![],
        source_scope: Some(scope),
        next_cursor,
        scan_complete: done,
        partial: partial || !limits.is_empty(),
        limits,
        scanned_events: inspected,
        source_bytes_read: source_bytes + context_bytes,
        freshness: None,
        window: None,
    })
}
