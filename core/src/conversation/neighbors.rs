//! Nearby visible messages in physical conversation order. Tool facts only
//! contribute frame bounds and never consume a requested message position.
use super::*;
use crate::{
    session_events::Position,
    usage_store::{EventChunk, EventPartition, ThreadEntry},
};
use std::cmp::{Ordering as Cmp, Reverse};

const MAX_FRAMES: usize = 192;
const MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA: usize = 10_000;
const MAX_NEIGHBORS: usize = 128;
const NEIGHBOR_BYTES: usize = 1024;
type MessageRows = Vec<(usize, Arc<Event>)>;
type Place = (u64, u32);
type Frame<'a> = (&'a EventPartition, &'a EventChunk, Place, Place);
type FileKey = (String, String, String, String);

struct Frames<'a> {
    before: Vec<Frame<'a>>,
    after: Vec<Frame<'a>>,
    partial: bool,
}

fn file(p: &Position) -> (&str, &str, &str) {
    (&p.source_instance_id, &p.file_id, &p.generation)
}

fn place(p: &Position) -> Place {
    (p.byte_offset, p.ordinal)
}

fn native_id(event: &Event) -> Option<&str> {
    match event.payload() {
        Payload::Message { native_id, .. } => native_id.as_deref(),
        _ => None,
    }
}

struct Nearby<'a> {
    snapshot: &'a Snapshot,
    request: Request,
    projects: &'a BTreeMap<&'a str, &'a ThreadEntry>,
    calendar: &'a Calendar,
    cancelled: &'a AtomicBool,
    frames: BTreeMap<&'a str, MessageRows>,
    files: BTreeMap<FileKey, Arc<Frames<'a>>>,
    frame_bytes: u64,
    metadata: usize,
    bodies: BTreeMap<String, bool>,
}

impl<'a> Nearby<'a> {
    fn frame(
        &mut self,
        partition: &'a EventPartition,
        chunk: &'a EventChunk,
    ) -> Result<Option<&MessageRows>> {
        let key = chunk.file.file.as_str();
        if !self.frames.contains_key(key) {
            if self.frames.len() >= MAX_FRAMES
                || self.frame_bytes.saturating_add(chunk.bytes) > MAX_FRAME_BYTES
            {
                return Ok(None);
            }
            let rows = self
                .snapshot
                .message_chunk(partition, chunk, self.cancelled)?;
            self.frame_bytes += chunk.bytes;
            self.frames.insert(key, rows);
        }
        Ok(self.frames.get(key))
    }

    fn file_frames(&mut self, thread: &str, origin: &Position) -> Result<Arc<Frames<'a>>> {
        let (source, path, generation) = file(origin);
        let key = (thread.into(), source.into(), path.into(), generation.into());
        if let Some(index) = self.files.get(&key) {
            return Ok(index.clone());
        }
        let partitions = &self.snapshot.event_index()?.partitions;
        let lo = partitions.partition_point(|p| p.target.thread_id.as_deref() < Some(thread));
        let hi = partitions.partition_point(|p| p.target.thread_id.as_deref() <= Some(thread));
        let mut frames = Vec::new();
        let mut partial = false;
        'metadata: for part in &partitions[lo..hi] {
            if !partition_in_scope(part, &self.request.selection, self.projects) {
                continue;
            }
            for chunk in &part.chunks {
                check(self.cancelled)?;
                if self.metadata == MAX_METADATA {
                    partial = true;
                    break 'metadata;
                }
                self.metadata += 1;
                if chunk.message_count == 0
                    || !self.calendar.matches_chunk(&self.request.selection, chunk)
                    || file(&chunk.first) > file(origin)
                    || file(&chunk.last) < file(origin)
                {
                    continue;
                }
                let first = if file(&chunk.first) == file(origin) {
                    place(&chunk.first)
                } else {
                    (0, 0)
                };
                let last = if file(&chunk.last) == file(origin) {
                    place(&chunk.last)
                } else {
                    (u64::MAX, u32::MAX)
                };
                frames.push((part.as_ref(), chunk, first, last));
            }
        }
        let mut before = frames.clone();
        before.sort_unstable_by_key(|(_, _, _, end)| Reverse(*end));
        frames.sort_unstable_by_key(|(_, _, start, _)| *start);
        let index = Arc::new(Frames {
            before,
            after: frames,
            partial,
        });
        self.files.insert(key, index.clone());
        Ok(index)
    }

    fn around(&mut self, anchor: &Event, count: usize) -> Result<(MessageRows, MessageRows, bool)> {
        let Some(thread) = anchor.thread_id() else {
            return Ok((vec![], vec![], true));
        };
        let origin = anchor.position();
        let index = self.file_frames(thread, origin)?;
        let sides = [&index.before, &index.after];
        let mut partial = index.partial;
        let mut windows = [Vec::new(), Vec::new()];
        for (side, (frames, window)) in sides.into_iter().zip(&mut windows).enumerate() {
            let compare = |a, b| {
                if side == 0 {
                    Ord::cmp(&b, &a)
                } else {
                    Ord::cmp(&a, &b)
                }
            };
            for &(partition, chunk, start, end) in frames {
                let point = place(origin);
                if (side == 0 && start >= point) || (side == 1 && end <= point) {
                    continue;
                }
                let bound = if side == 0 {
                    end.min(point)
                } else {
                    start.max(point)
                };
                if window.len() == count {
                    let (_, last): &(usize, Arc<Event>) = window.last().unwrap();
                    if compare(bound, place(last.position())) != Cmp::Less {
                        break;
                    }
                }
                // Copy the small Arc list so scope checks can borrow the same reader.
                let Some(rows) = self.frame(partition, chunk)?.cloned() else {
                    partial = true;
                    break;
                };
                for (offset, e) in rows {
                    let direction = place(e.position()).cmp(&place(origin));
                    if file(e.position()) == file(origin)
                        && direction == if side == 0 { Cmp::Less } else { Cmp::Greater }
                        && !matches!(
                            e.payload(),
                            Payload::Message {
                                record_kind: crate::session_events::MessageRecordKind::Delta,
                                ..
                            }
                        )
                        && !native_id(anchor).is_some_and(|id| native_id(&e) == Some(id))
                        && e.original_content().is_some_and(|c| !c.chunks.is_empty())
                        && selected(
                            &e,
                            &self.request.selection,
                            self.projects,
                            self.calendar,
                            None,
                        )?
                    {
                        window.push((offset, e));
                    }
                }
                window.sort_unstable_by(|(_, a), (_, b)| {
                    compare(place(a.position()), place(b.position()))
                });
                let mut ids = std::collections::BTreeSet::new();
                window.retain(|(_, e)| native_id(e).is_none_or(|id| ids.insert(id.to_owned())));
                window.truncate(count);
            }
        }
        let [mut before, after] = windows;
        before.reverse();
        Ok((before, after, partial))
    }

    fn materialize(
        &mut self,
        rows: MessageRows,
        reader: &mut Reader<'_>,
        output_budget: &mut usize,
        pool: &mut Vec<Neighbor>,
    ) -> Result<(Vec<String>, bool)> {
        let mut ids = vec![];
        let mut partial = false;
        for (offset, e) in rows {
            let id = e.id();
            if let Some(&available) = self.bodies.get(id) {
                if available {
                    ids.push(id.to_owned());
                } else {
                    partial = true;
                }
                continue;
            }
            if pool.len() >= MAX_NEIGHBORS {
                partial = true;
                break;
            }
            let target = ReadTarget {
                evidence: activity(&e, offset, self.projects, None, None).evidence,
                cursor: None,
                max_bytes: Some(NEIGHBOR_BYTES),
                before: 0,
                after: 0,
            };
            let body = match fragment_for_event(
                self.snapshot,
                &self.request,
                &target,
                self.projects,
                reader,
                self.cancelled,
                self.calendar,
                None,
                &e,
            ) {
                Ok(body) => body,
                Err(err) if read_issue(&err).is_some() => {
                    partial = true;
                    self.bodies.insert(id.to_owned(), false);
                    continue;
                }
                Err(err) => return Err(err),
            };
            let neighbor = Neighbor {
                message: body.message,
                text: body.text,
                locations: body.locations,
                partial: body.partial || body.next_cursor.is_some(),
            };
            let bytes = serde_json::to_vec(&neighbor)?.len() + 16;
            if bytes > *output_budget {
                partial = true;
                self.bodies.insert(id.to_owned(), false);
                continue;
            }
            *output_budget -= bytes;
            ids.push(id.to_owned());
            self.bodies.insert(id.to_owned(), true);
            pool.push(neighbor);
        }
        Ok((ids, partial))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn attach(
    snapshot: &Snapshot,
    request: &Request,
    projects: &BTreeMap<&str, &ThreadEntry>,
    calendar: &Calendar,
    reader: &mut Reader<'_>,
    cancelled: &AtomicBool,
    count: usize,
    output_budget: &mut usize,
    response: &mut Response,
) -> Result<()> {
    let mut visible = request.clone();
    // The matched speaker is a search predicate. A requested conversation
    // window includes both sides, within the original date and identity filters.
    visible.selection.origins.clear();
    let mut nearby = Nearby {
        snapshot,
        request: visible,
        projects,
        calendar,
        cancelled,
        frames: BTreeMap::new(),
        files: BTreeMap::new(),
        frame_bytes: 0,
        metadata: 0,
        bodies: BTreeMap::new(),
    };
    for f in &mut response.fragments {
        let occurrences = std::iter::once((&f.evidence, &mut f.visible_context)).chain(
            f.also_at
                .iter_mut()
                .map(|o| (&o.evidence, &mut o.visible_context)),
        );
        for (evidence, context) in occurrences {
            check(cancelled)?;
            let refs_budget = 80 + count * 2 * 68;
            if *output_budget < refs_budget {
                response.partial = true;
                response.limits.push("context_budget".into());
                return Ok(());
            }
            *output_budget -= refs_budget;
            let anchor = event(snapshot, evidence, cancelled)?;
            let (before, after, incomplete) = nearby.around(&anchor, count)?;
            let (before, left_missing) =
                nearby.materialize(before, reader, output_budget, &mut response.neighbors)?;
            let (after, right_missing) =
                nearby.materialize(after, reader, output_budget, &mut response.neighbors)?;
            let partial = incomplete || left_missing || right_missing;
            response.partial |= partial;
            *context = Some(VisibleContext {
                before,
                after,
                partial,
            });
        }
    }
    Ok(())
}
