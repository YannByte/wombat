//! Read-only visible text search. It owns no product storage or ledger.
mod checkpoint;
mod context;
mod read;
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
    partial: bool,
}

struct SourceFile {
    source: String,
    root: String,
    path: PathBuf,
    meta: FileMetadata,
    reference: SourceSearchFile,
    header_end: u64,
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

fn inventory(request: &Request) -> (Vec<SourceFile>, bool, u64) {
    let discovered = CodexAdapter.discover(&DiscoveryRequest {
        roots: request.roots.iter().map(PathBuf::from).collect(),
    });
    let mut partial = !discovered.issues.is_empty();
    let mut files = Vec::new();
    let mut bytes = 0;
    let mut visited_files = 0;
    'sources: for source in discovered.sources {
        if !request.selection.sources.is_empty() && !request.selection.sources.contains(&source.id)
        {
            continue;
        }
        let root = Path::new(&source.root);
        let context = RunContext {
            max_files: MAX_FILES.saturating_sub(visited_files),
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
        visited_files += paths.len();
        for path in paths {
            if bytes >= MAX_SOURCE_BYTES {
                partial = true;
                break 'sources;
            }
            match source_file(&source, &path, &mut bytes) {
                Ok(file)
                    if (request.selection.projects.is_empty()
                        || file
                            .reference
                            .project
                            .as_ref()
                            .is_some_and(|p| request.selection.projects.contains(p)))
                        && (request.selection.threads.is_empty()
                            || request
                                .selection
                                .threads
                                .contains(&file.reference.thread_id)) =>
                {
                    files.push(file);
                }
                Ok(_) => {}
                Err(_) => partial = true,
            }
        }
    }
    (files, partial, bytes)
}

fn is_rollout(root: &Path, path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
        && ["sessions", "archived_sessions"].iter().any(|d| {
            path.strip_prefix(root.join(d)).is_ok_and(|p| {
                p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            })
        })
}

fn source_file(source: &SourceInstance, path: &Path, bytes: &mut u64) -> Result<SourceFile> {
    let path = dunce::canonicalize(path)?;
    anyhow::ensure!(is_rollout(Path::new(&source.root), &path));
    anyhow::ensure!(fs::symlink_metadata(&path)?.is_file());
    let file = File::open(&path)?;
    let meta = FileMetadata::from_file(&file)?;
    anyhow::ensure!(meta.is_file());
    let mut line = Vec::new();
    let header_end = BufReader::with_capacity(4096, file.take(meta.len()))
        .take(MAX_LINE_BYTES)
        .read_until(b'\n', &mut line)? as u64;
    *bytes += header_end;
    anyhow::ensure!(line.ends_with(b"\n"));
    let envelope: Envelope<'_> = serde_json::from_slice(&line)?;
    anyhow::ensure!(envelope.kind == "session_meta");
    let header: Header<'_> = serde_json::from_str(envelope.payload.get())?;
    let upstream = header
        .id
        .or(header.session_id)
        .ok_or_else(|| anyhow::anyhow!("Missing Codex session identity"))?;
    let reference = SourceSearchFile {
        source_instance_id: source.id.clone(),
        thread_id: thread_identity(&source.id, &upstream),
        upstream_id: upstream.into_owned(),
        project: header.cwd.map(Cow::into_owned),
        file: path.to_string_lossy().into_owned(),
        generation: crate::adapters::stable_id(&[&source.id, &meta.physical]),
    };
    Ok(SourceFile {
        source: source.id.clone(),
        root: source.root.clone(),
        path,
        meta,
        reference,
        header_end,
    })
}

fn open(file: &SourceFile) -> Result<File> {
    // Recheck containment before reopening a path learned from discovery.
    let path = dunce::canonicalize(&file.path)?;
    if path != file.path || !path.starts_with(Path::new(&file.root)) {
        return Err(err("SOURCE_CHANGED", "Log path left its source root"));
    }
    let opened = File::open(path)?;
    let observed = FileMetadata::from_file(&opened)?;
    if observed.physical != file.meta.physical
        || observed.len() < file.meta.len()
        || observed.len() == file.meta.len()
            && observed.modified().ok() != file.meta.modified().ok()
    {
        return Err(err(
            "SOURCE_CHANGED",
            "Log was replaced, truncated or rewritten",
        ));
    }
    Ok(opened)
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

fn message_prefix(parts: &[TextPart<'_>], cap: usize) -> (String, bool) {
    let mut text = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            if text.len() == cap {
                return (text, true);
            }
            text.push('\n');
        }
        let mut end = part.text.len().min(cap - text.len());
        while !part.text.is_char_boundary(end) {
            end -= 1;
        }
        text.push_str(&part.text[..end]);
        if end < part.text.len() {
            return (text, true);
        }
    }
    (text, false)
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

struct Calendar {
    since: Option<chrono::NaiveDate>,
    until: Option<chrono::NaiveDate>,
    zone: chrono_tz::Tz,
    include_undated: bool,
}
impl Calendar {
    fn new(selection: &Selection) -> Result<Self> {
        let date = |value: &Option<String>| {
            value
                .as_deref()
                .map(crate::usage_app::scope::date)
                .transpose()
        };
        let (since, until) = (date(&selection.since)?, date(&selection.until)?);
        if since.is_some() != until.is_some() || since.zip(until).is_some_and(|(a, b)| a >= b) {
            return Err(err(
                "INVALID_ARGUMENT",
                "Set both dates and place the start before the exclusive end",
            ));
        }
        let zone = crate::usage_app::scope::timezone(&crate::usage_app_dto::Scope {
            timezone: selection.timezone.clone(),
            ..Default::default()
        })?;
        Ok(Self {
            since,
            until,
            zone,
            include_undated: selection.include_undated,
        })
    }

    fn contains(&self, time: Option<&str>) -> bool {
        let day = time
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&self.zone).date_naive());
        match day {
            Some(day) => self.since.is_none_or(|x| day >= x) && self.until.is_none_or(|x| day < x),
            None => self.since.is_none() || self.include_undated,
        }
    }
}

pub(crate) fn query(request: &Request) -> Result<Response> {
    if request.action == Action::SourceRead {
        return read::query(request);
    }
    let scan = request.action == Action::SourceScan;
    let s = &request.selection;
    let cap = request.excerpt_bytes.unwrap_or(8192);
    let limit = request.limit.unwrap_or(200);
    let terms: Vec<&str> = request
        .query
        .iter()
        .chain(request.terms.iter().flatten())
        .map(String::as_str)
        .collect();
    if (!scan && (terms.is_empty() || request.checkpoint.is_some()))
        || (scan
            && (!terms.is_empty() || s.projects.len() != 1 || request.context_messages.is_some()))
        || !request.source_targets.is_empty()
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
    let calendar = Calendar::new(s)?;
    let in_calendar = |time: Option<&str>| calendar.contains(time);
    let (files, mut partial, mut source_bytes) = inventory(request);
    let baseline = if scan {
        Some(checkpoint::open(request, &files, &mut source_bytes)?)
    } else {
        None
    };
    let mut scope = Sha256::new();
    scope.update(serde_json::to_vec(&(
        2,
        request.action,
        &request.roots,
        s,
        &request.checkpoint,
    ))?);
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
    let binding = crate::hash(serde_json::to_vec(&(
        &scope,
        &terms,
        request.context_messages,
        request.excerpt_bytes,
    ))?);
    let mut cursor = match &request.cursor {
        Some(value) => serde_json::from_str::<Cursor>(value)
            .map_err(|_| err("INVALID_ARGUMENT", "Malformed direct search cursor"))?,
        None => Cursor {
            binding: binding.clone(),
            file: 0,
            offset: 0,
            partial: false,
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
    let mut inspected = 0;
    let mut rows = 0;
    // Leave room for the echoed request or completed checkpoint.
    let mut output =
        serde_json::to_vec(request)?.len() + 8192 + if scan { checkpoint::MAX_BYTES } else { 0 };
    let mut limits = Vec::new();
    let started = Instant::now();
    let mut line = Vec::new();
    'files: while let Some(f) = files.get(cursor.file) {
        if source_bytes >= MAX_SOURCE_BYTES || started.elapsed().as_millis() >= MAX_SCAN_MILLIS {
            limits.push("source_scan_budget".into());
            break;
        }
        let header_n = f.header_end;
        let upstream = &f.reference.upstream_id;
        let mut offset = cursor
            .offset
            .max(header_n)
            .max(baseline.as_ref().map_or(0, |baseline| baseline.offset(f)));
        if offset == f.meta.len() {
            cursor.file += 1;
            cursor.offset = 0;
            continue;
        }
        if offset > f.meta.len() {
            return Err(err(
                "SOURCE_CHANGED",
                "Direct cursor is outside the source file",
            ));
        }
        let mut file = open(f)?;
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
                partial: message_partial,
                ..
            } = match origin_and_text(&envelope, upstream) {
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
            let (text, matched_term, match_range, truncated) = if scan {
                if parts.is_empty() {
                    continue;
                }
                let (text, truncated) = message_prefix(&parts, cap);
                (text, None, None, truncated)
            } else {
                let Some((part, start, term)) = parts.iter().find_map(|part| {
                    query
                        .iter()
                        .enumerate()
                        .filter_map(|(i, finder)| {
                            finder.find(part.text.as_bytes()).map(|pos| (pos, i))
                        })
                        .min()
                        .map(|(start, index)| (part, start, terms[index]))
                }) else {
                    continue;
                };
                let (text, range) = excerpt(&part.text, start, term.len(), cap);
                let truncated = text.len() < part.text.len() || parts.len() > 1;
                (text, Some(term.into()), Some(range), truncated)
            };
            let new_file = file_index.is_none().then(|| f.reference.clone());
            let hit = SourceHit {
                file_index: file_index.unwrap_or(source_files.len()),
                timestamp: envelope.timestamp.map(str::to_owned),
                origin: speaker,
                byte_start: at,
                byte_end: offset,
                sha256: crate::hash(&line),
                matched_term,
                match_range,
                text,
                truncated: truncated || message_partial,
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
                upstream,
                &in_calendar,
                &mut output,
            )?;
        }
        let after = FileMetadata::read(&f.path)?;
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
    cursor.partial |= partial;
    let next_checkpoint = if scan && done && !cursor.partial {
        Some(checkpoint::capture(request, &files, &mut source_bytes)?)
    } else {
        None
    };
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
        action: request.action,
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
        source_fragments: vec![],
        source_read_issues: vec![],
        next_checkpoint,
        next_search,
        read_issues: vec![],
        source_scope: Some(scope),
        next_cursor,
        scan_complete: done,
        partial: cursor.partial || !limits.is_empty(),
        limits,
        scanned_events: inspected,
        source_bytes_read: source_bytes + context_bytes,
        freshness: None,
        window: None,
    })
}
