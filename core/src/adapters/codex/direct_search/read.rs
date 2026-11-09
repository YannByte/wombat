//! Seek to verified visible JSONL records without building a conversation index.
use super::*;

const MAX_BATCH_INPUT: usize = 512 * 1024;
const MAX_BATCH_READ: u64 = 64 * 1024 * 1024;

fn selected<T: PartialEq>(choices: &[T], value: &T) -> bool {
    choices.is_empty() || choices.contains(value)
}

fn read_one(
    file: &SourceFile,
    target: &SourceReadTarget,
    selection: &Selection,
    calendar: &super::Calendar,
    cap: usize,
    bytes: &mut u64,
) -> Result<(
    String,
    bool,
    Option<String>,
    crate::session_events::MessageOrigin,
)> {
    if file.reference != target.source
        || target.byte_end > file.meta.len()
        || target.byte_start < file.header_end
    {
        return Err(err(
            "SOURCE_CHANGED",
            "The original log or record range changed",
        ));
    }
    let count = target.byte_end - target.byte_start;
    let mut opened = super::open(file)?;
    opened.seek(SeekFrom::Start(target.byte_start - 1))?;
    let mut line = vec![0; count as usize + 1];
    opened.read_exact(&mut line)?;
    *bytes += line.len() as u64;
    let raw = &line[1..];
    if line[0] != b'\n'
        || raw.last() != Some(&b'\n')
        || raw[..raw.len() - 1].contains(&b'\n')
        || crate::hash(raw) != target.sha256
    {
        return Err(err(
            "SOURCE_CHANGED",
            "The selected original record changed",
        ));
    }
    let row: Envelope<'_> = serde_json::from_slice(raw)?;
    if !calendar.contains(row.timestamp) {
        return Err(err(
            "INVALID_ARGUMENT",
            "Record lies outside the selected dates",
        ));
    }
    let msg = origin_and_text(&row, &file.reference.upstream_id)
        .map_err(|_| err("SOURCE_UNREADABLE", "Unsupported source record"))?
        .filter(|m| !m.parts.is_empty() && selected(&selection.origins, &m.origin))
        .ok_or_else(|| err("NOT_FOUND", "No visible text in the selected record"))?;
    let (mut text, mut truncated) = message_prefix(&msg.parts, cap);
    if let Some(term) = &target.matched_term {
        let (part, offset) = msg
            .parts
            .iter()
            .find_map(|part| part.text.find(term).map(|offset| (part, offset)))
            .ok_or_else(|| {
                err(
                    "INVALID_ARGUMENT",
                    "Search term does not match the original record",
                )
            })?;
        if truncated && !text.contains(term.as_str()) {
            text = excerpt(&part.text, offset, term.len(), cap).0;
            truncated = true;
        }
    }
    Ok((
        text,
        truncated || msg.partial,
        row.timestamp.map(str::to_owned),
        msg.origin,
    ))
}

pub(super) fn query(request: &Request) -> Result<Response> {
    let cap = request.excerpt_bytes.unwrap_or(8192);
    let s = &request.selection;
    if !(1..=65536).contains(&cap)
        || request.source_targets.len() > MAX_READ_TARGETS
        || request.source_targets.is_empty()
        || serde_json::to_vec(request)?.len() > MAX_BATCH_INPUT
        || !request.targets.is_empty()
        || request.query.is_some()
        || request.terms.is_some()
        || request.checkpoint.is_some()
        || request.limit.is_some()
        || request.offset.is_some()
        || request.cursor.is_some()
        || request.source_scope.is_some()
        || request.fixed_scope
        || request.complete
        || request.context_messages.is_some()
        || request.anchor.is_some()
        || request.radius.is_some()
        || request.max_scan_bytes.is_some()
        || s.include_tool_output
        || s.operation_id.is_some()
        || !s.turns.is_empty()
        || !s.phases.is_empty()
        || !s.record_kinds.is_empty()
    {
        return Err(err(
            "INVALID_ARGUMENT",
            "Direct reads require bounded exact source targets",
        ));
    }
    let calendar = super::Calendar::new(s)?;
    let sources = CodexAdapter.discover(&DiscoveryRequest {
        roots: request.roots.iter().map(PathBuf::from).collect(),
    });
    let mut files = BTreeMap::<(&str, &str), Option<SourceFile>>::new();
    let mut bodies = BTreeMap::<String, usize>::new();
    let mut fragments = Vec::<SourceReadFragment>::new();
    let mut issues = Vec::new();
    let mut bytes = 0;
    let mut output =
        serde_json::to_vec(s)?.len() + serde_json::to_vec(&request.roots)?.len() + 24 * 1024;
    let started = Instant::now();
    for (index, target) in request.source_targets.iter().enumerate() {
        let mut issue = |code: &str, deferred| {
            issues.push(SourceReadIssue {
                target_index: index,
                code: code.into(),
                deferred,
            })
        };
        if target.byte_start >= target.byte_end
            || target.byte_end - target.byte_start > MAX_LINE_BYTES
            || target.sha256.len() != 64
            || !target.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || target
                .matched_term
                .as_ref()
                .is_some_and(|t| t.trim().is_empty() || t.len() > MAX_TERM_BYTES || t.len() > cap)
        {
            issue("INVALID_ARGUMENT", false);
            continue;
        }
        if bytes >= MAX_BATCH_READ || started.elapsed().as_millis() >= MAX_SCAN_MILLIS {
            issue("RESOURCE_LIMIT", true);
            continue;
        }
        let source_id = target.source.source_instance_id.as_str();
        let path = Path::new(&target.source.file);
        let Some(source) = sources.sources.iter().find(|r| {
            r.id == source_id && selected(&s.sources, &r.id) && is_rollout(Path::new(&r.root), path)
        }) else {
            issue("INVALID_ARGUMENT", false);
            continue;
        };
        let file = files
            .entry((source_id, &target.source.file))
            .or_insert_with(|| source_file(source, path, &mut bytes).ok());
        let Some(file) = file else {
            issue("SOURCE_UNREADABLE", false);
            continue;
        };
        if !selected(&s.threads, &file.reference.thread_id)
            || !s.projects.is_empty()
                && !file
                    .reference
                    .project
                    .as_ref()
                    .is_some_and(|p| s.projects.contains(p))
        {
            issue("INVALID_ARGUMENT", false);
            continue;
        }
        let (text, truncated, timestamp, origin) =
            match read_one(file, target, s, &calendar, cap, &mut bytes) {
                Ok(value) => value,
                Err(error) => {
                    let code = error
                        .downcast_ref::<crate::dto::OperationError>()
                        .map_or("SOURCE_UNREADABLE", |e| e.code);
                    issue(
                        match code {
                            "SOURCE_CHANGED" | "INVALID_ARGUMENT" | "NOT_FOUND" => code,
                            _ => "SOURCE_UNREADABLE",
                        },
                        false,
                    );
                    continue;
                }
            };
        let occurrence = SourceReadOccurrence {
            target_index: index,
            timestamp,
            origin,
        };
        let key = (!truncated).then(|| crate::hash(text.as_bytes()));
        if let Some(existing) = key.as_ref().and_then(|k| bodies.get(k)).copied()
            && fragments[existing].text == text
        {
            let size = serde_json::to_vec(&occurrence)?.len() + 1;
            if output + size > MAX_OUTPUT_BYTES {
                issue("OUTPUT_LIMIT", true);
                continue;
            }
            output += size;
            fragments[existing].occurrences.push(occurrence);
            continue;
        }
        let fragment = SourceReadFragment {
            text,
            truncated,
            occurrences: vec![occurrence],
        };
        let size = serde_json::to_vec(&fragment)?.len() + 1;
        if output + size > MAX_OUTPUT_BYTES {
            issue("OUTPUT_LIMIT", true);
            continue;
        }
        output += size;
        if let Some(key) = key {
            bodies.insert(key, fragments.len());
        }
        fragments.push(fragment);
    }
    Ok(Response {
        record: None,
        output_version: OUTPUT_VERSION,
        action: Action::SourceRead,
        snapshot_id: format!("source-read:{}", crate::hash(serde_json::to_vec(request)?)),
        baseline_snapshot_id: None,
        selection: s.clone(),
        roots: request.roots.clone(),
        items: vec![],
        fragments: vec![],
        neighbors: vec![],
        source_hits: vec![],
        source_files: vec![],
        source_neighbors: vec![],
        source_fragments: fragments,
        partial: !issues.is_empty(),
        scan_complete: !issues.iter().any(|i| i.deferred),
        source_read_issues: issues,
        next_checkpoint: None,
        next_search: None,
        read_issues: vec![],
        source_scope: None,
        next_cursor: None,
        limits: vec![],
        scanned_events: 0,
        source_bytes_read: bytes,
        freshness: None,
        window: None,
    })
}
