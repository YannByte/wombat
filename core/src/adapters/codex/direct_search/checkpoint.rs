//! Caller-owned progress for append-only Codex logs.
use super::*;

pub(super) const MAX_BYTES: usize = 256 * 1024;
const MAX_TRACKED_FILES: usize = 1024;
const WITNESS_BYTES: u64 = 4096;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    binding: String,
    files: BTreeMap<String, Boundary>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Boundary {
    generation: String,
    offset: u64,
    modified: Option<u128>,
    witness: String,
}

fn binding(request: &Request) -> Result<String> {
    Ok(crate::hash(serde_json::to_vec(&(
        1,
        &request.roots,
        &request.selection,
    ))?))
}

fn key(file: &SourceFile) -> String {
    crate::adapters::stable_id(&[&file.source, &file.reference.file])
}

pub(super) fn modified(meta: &FileMetadata) -> Option<u128> {
    meta.modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
}

fn witness(file: &SourceFile, offset: u64, bytes: &mut u64) -> Result<String> {
    let mut input = super::open(file)?;
    let count = offset.min(WITNESS_BYTES);
    input.seek(SeekFrom::Start(offset - count))?;
    let mut sample = vec![0; count as usize];
    input.read_exact(&mut sample)?;
    *bytes += count;
    if sample.last() != Some(&b'\n') {
        return Err(err(
            "SOURCE_CHANGED",
            "Checkpoint must end at a complete log record",
        ));
    }
    Ok(crate::hash(sample))
}

pub(super) fn open(request: &Request, files: &[SourceFile], bytes: &mut u64) -> Result<Checkpoint> {
    if files.len() > MAX_TRACKED_FILES {
        return Err(err(
            "RESOURCE_LIMIT",
            "Project checkpoint exceeds the file limit",
        ));
    }
    let Some(raw) = &request.checkpoint else {
        return Ok(Checkpoint::default());
    };
    if raw.len() > MAX_BYTES {
        return Err(err(
            "RESOURCE_LIMIT",
            "Project checkpoint exceeds the byte limit",
        ));
    }
    let previous: Checkpoint = serde_json::from_str(raw)
        .map_err(|_| err("INVALID_ARGUMENT", "Invalid project checkpoint"))?;
    if previous.binding != binding(request)? || previous.files.len() > MAX_TRACKED_FILES {
        return Err(err(
            "INVALID_ARGUMENT",
            "Checkpoint requires the original roots and project filters",
        ));
    }
    let mut matched = 0;
    for file in files {
        let Some(prior) = previous.files.get(&key(file)) else {
            continue;
        };
        matched += 1;
        if prior.generation != file.reference.generation
            || prior.offset < file.header_end
            || prior.offset > file.meta.len()
            || prior.offset == file.meta.len() && prior.modified != modified(&file.meta)
            || prior.witness != witness(file, prior.offset, bytes)?
        {
            return Err(err(
                "SOURCE_CHANGED",
                "A checkpoint log was replaced, truncated or rewritten. Start a new project scan.",
            ));
        }
    }
    if matched != previous.files.len() {
        return Err(err(
            "SOURCE_CHANGED",
            "A checkpoint log is no longer in the selected project. Start a new project scan.",
        ));
    }
    Ok(previous)
}

impl Checkpoint {
    pub(super) fn offset(&self, file: &SourceFile) -> u64 {
        self.files.get(&key(file)).map_or(0, |entry| entry.offset)
    }
}

pub(super) fn capture(request: &Request, files: &[SourceFile], bytes: &mut u64) -> Result<String> {
    let mut progress = Checkpoint {
        binding: binding(request)?,
        files: BTreeMap::new(),
    };
    for file in files {
        let offset = file.meta.len();
        progress.files.insert(
            key(file),
            Boundary {
                generation: file.reference.generation.clone(),
                offset,
                modified: modified(&file.meta),
                witness: witness(file, offset, bytes)?,
            },
        );
    }
    let raw = serde_json::to_string(&progress)?;
    if raw.len() > MAX_BYTES {
        return Err(err(
            "RESOURCE_LIMIT",
            "Project checkpoint exceeds the byte limit",
        ));
    }
    Ok(raw)
}
