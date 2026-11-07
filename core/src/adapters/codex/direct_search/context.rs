//! Find adjacent visible messages in one bounded pass per hit file.
use super::*;
use std::collections::VecDeque;

const MAX_NEIGHBORS: usize = 128;
const NEIGHBOR_BYTES: usize = 1024;

struct Row {
    body: SourceNeighbor,
    native_id: Option<String>,
}
struct Window {
    native_id: Option<String>,
    before: Vec<Arc<Row>>,
    after: Vec<Arc<Row>>,
}

pub(super) struct ReadContext {
    count: usize,
    bytes: u64,
    time: std::time::Duration,
    neighbors: Vec<SourceNeighbor>,
    refs: BTreeMap<(usize, u64), usize>,
    partial: bool,
}

impl ReadContext {
    pub(super) fn new(count: usize) -> Self {
        Self {
            count,
            bytes: 0,
            time: Default::default(),
            neighbors: vec![],
            refs: BTreeMap::new(),
            partial: false,
        }
    }

    pub(super) fn finish(self) -> (Vec<SourceNeighbor>, u64, bool) {
        (self.neighbors, self.bytes, self.partial)
    }

    fn index(&mut self, row: Arc<Row>, output: &mut usize) -> Result<Option<usize>> {
        let key = (row.body.file_index, row.body.byte_start);
        if let Some(&index) = self.refs.get(&key) {
            return Ok(Some(index));
        }
        let size = serde_json::to_vec(&row.body)?.len() + 1;
        if self.neighbors.len() >= MAX_NEIGHBORS || *output + size > MAX_OUTPUT_BYTES {
            return Ok(None);
        }
        let index = self.neighbors.len();
        self.refs.insert(key, index);
        self.neighbors.push(row.body.clone());
        *output += size;
        Ok(Some(index))
    }

    pub(super) fn attach(
        &mut self,
        file: impl Read,
        mut offset: u64,
        hits: &mut [SourceHit],
        upstream: &str,
        in_calendar: &impl Fn(Option<&str>) -> bool,
        output: &mut usize,
    ) -> Result<()> {
        let started = Instant::now();
        let mut reader = BufReader::with_capacity(1024 * 1024, file);
        let mut previous = VecDeque::<Arc<Row>>::new();
        let mut waiting = Vec::<usize>::new();
        let mut windows = Vec::<Window>::new();
        let mut line = Vec::new();
        let mut incomplete = false;
        let mut rows = 0;
        let file_index = hits[0].file_index;
        while windows.len() < hits.len() || !waiting.is_empty() {
            rows += 1;
            if self.bytes >= MAX_SOURCE_BYTES
                || rows & 1023 == 1
                    && (self.time + started.elapsed()).as_millis() >= MAX_SCAN_MILLIS
            {
                incomplete = true;
                break;
            }
            line.clear();
            let bytes = reader
                .by_ref()
                .take(MAX_LINE_BYTES + 1)
                .read_until(b'\n', &mut line)? as u64;
            if bytes == 0 {
                break;
            }
            self.bytes += bytes;
            let start = offset;
            offset += bytes;
            if !line.ends_with(b"\n") {
                incomplete = true;
                break;
            }
            if compact_candidate(&line, true, true) == Some(false) {
                continue;
            }
            let Ok(envelope) = serde_json::from_slice::<Envelope>(&line) else {
                incomplete = true;
                continue;
            };
            if !in_calendar(envelope.timestamp) {
                continue;
            }
            let message = match origin_and_text(&envelope, upstream) {
                Ok(Some(message)) if !message.parts.is_empty() => message,
                Ok(_) => continue,
                Err(_) => {
                    incomplete = true;
                    continue;
                }
            };
            let native_id = message.native_id.map(Cow::into_owned);
            let matches_anchor = hits
                .get(windows.len())
                .is_some_and(|hit| hit.byte_start == start);
            let digest = crate::hash(&line);
            if matches_anchor && hits[windows.len()].sha256 != digest {
                return Err(err(
                    "SOURCE_CHANGED",
                    "A search hit changed during context reading",
                ));
            }
            let (text, partial) = prefix(&message.parts);
            let row = Arc::new(Row {
                native_id,
                body: SourceNeighbor {
                    file_index,
                    timestamp: envelope.timestamp.map(str::to_owned),
                    origin: message.origin,
                    byte_start: start,
                    byte_end: offset,
                    sha256: digest,
                    text,
                    partial: partial || message.partial,
                },
            });
            waiting.retain(|&i| {
                let window = &mut windows[i];
                if !same_id(&window.native_id, &row.native_id)
                    && !window
                        .after
                        .iter()
                        .any(|seen| same_id(&seen.native_id, &row.native_id))
                {
                    window.after.push(row.clone());
                }
                window.after.len() < self.count
            });
            if matches_anchor {
                let mut before: Vec<_> = previous
                    .iter()
                    .rev()
                    .filter(|seen| !same_id(&seen.native_id, &row.native_id))
                    .take(self.count)
                    .cloned()
                    .collect();
                before.reverse();
                waiting.push(windows.len());
                windows.push(Window {
                    native_id: row.native_id.clone(),
                    before,
                    after: vec![],
                });
            }
            previous.retain(|seen| !same_id(&seen.native_id, &row.native_id));
            previous.push_back(row);
            if previous.len() > self.count + 1 {
                previous.pop_front();
            }
        }
        self.time += started.elapsed();
        if windows.len() != hits.len() && !incomplete {
            return Err(err(
                "SOURCE_CHANGED",
                "A search hit disappeared during context reading",
            ));
        }
        for (i, hit) in hits.iter_mut().enumerate() {
            let mut context = SourceContext {
                before: vec![],
                after: vec![],
                partial: incomplete,
            };
            if *output + 128 <= MAX_OUTPUT_BYTES {
                *output += 128;
                if let Some(window) = windows.get_mut(i) {
                    for (rows, refs) in [
                        (&mut window.before, &mut context.before),
                        (&mut window.after, &mut context.after),
                    ] {
                        for row in rows.drain(..) {
                            if let Some(index) = self.index(row, output)? {
                                refs.push(index);
                            } else {
                                context.partial = true;
                            }
                        }
                    }
                }
                self.partial |= context.partial;
                hit.context = Some(context);
            } else {
                self.partial = true;
            }
        }
        Ok(())
    }
}

fn same_id(a: &Option<String>, b: &Option<String>) -> bool {
    a.is_some() && a == b
}

fn prefix(parts: &[TextPart<'_>]) -> (String, bool) {
    let mut text = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            let remaining = NEIGHBOR_BYTES - text.len();
            if remaining == 0 || !text.is_empty() && part.encoded_bytes + 1 > remaining {
                return (text, true);
            }
            text.push('\n');
        }
        let part = &part.text;
        let mut end = part.len().min(NEIGHBOR_BYTES - text.len());
        while !part.is_char_boundary(end) {
            end -= 1;
        }
        text.push_str(&part[..end]);
        if end < part.len() {
            return (text, true);
        }
    }
    (text, false)
}
