//! Optional one-shot reuse for measured, stable historical accounting input.
mod platform;
use super::*;
use crate::adapters::codex::usage_input::{self, Inventory};
use std::{
    cell::RefCell,
    io::{self, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const FILE: &str = "ledger-v1.zst";
const ADMISSION: &str = "admission-v1.json";
const LOCK: &str = "build.lock";
const SIGNATURE: &str = "wombat-ledger-v1";
const MIN_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENCODED: usize = 16 * 1024 * 1024;
const MAX_DECODED: usize = 64 * 1024 * 1024;
const MAX_ADMISSION: usize = 8 * 1024;
const MIN_READ: Duration = Duration::from_secs(1);
const BUILD_LIMIT: Duration = Duration::from_secs(45);
const MEMORY_LIMIT: u64 = 384 * 1024 * 1024;

thread_local! {
    static JOB: RefCell<Option<uuid::Uuid>> = const { RefCell::new(None) };
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    fingerprint: String,
    roots: Vec<String>,
    attempt: Option<String>,
}

fn root() -> Result<PathBuf> {
    Ok(crate::storage::data_home()?.join("usage-reuse-v1"))
}

pub(super) fn candidate(roots: &[String], recent_only: bool) -> Option<Inventory> {
    if recent_only || !platform::SUPPORTED {
        return None;
    }
    usage_input::capture(roots)
        .ok()
        .flatten()
        .filter(|i| i.bytes >= MIN_BYTES)
}

fn safe_file(path: &Path, size: usize) -> Result<fs::File> {
    let meta = fs::symlink_metadata(path)?;
    anyhow::ensure!(meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= size as u64);
    Ok(fs::File::open(path)?)
}

pub(super) fn load(input: &Inventory) -> Option<Collected> {
    load_file(&root().ok()?.join(FILE), &input.fingerprint)
}

fn load_file(path: &Path, fingerprint: &str) -> Option<Collected> {
    let mut file = BufReader::new(safe_file(path, MAX_ENCODED).ok()?);
    let expected = format!("{SIGNATURE}:{fingerprint}\n");
    let mut header = vec![0; expected.len()];
    file.read_exact(&mut header).ok()?;
    if header != expected.as_bytes() {
        return None;
    }
    let mut decoder = zstd::stream::read::Decoder::with_buffer(file).ok()?;
    decoder.window_log_max(23).ok()?;
    let mut raw = Vec::new();
    decoder
        .take(MAX_DECODED as u64 + 1)
        .read_to_end(&mut raw)
        .ok()?;
    if raw.len() > MAX_DECODED {
        return None;
    }
    serde_json::from_slice(&raw).ok()
}

fn lock(path: &Path) -> Result<Option<fs::File>> {
    let path = path.join(LOCK);
    crate::storage::private_file(&path)?;
    let file = fs::OpenOptions::new().read(true).write(true).open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn admission(root: &Path) -> Option<Admission> {
    let file = safe_file(&root.join(ADMISSION), MAX_ADMISSION).ok()?;
    serde_json::from_reader(file.take(MAX_ADMISSION as u64)).ok()
}

pub(super) fn observed(roots: &[String], input: &Inventory, elapsed: Duration) -> Result<()> {
    if elapsed < MIN_READ {
        return Ok(());
    }
    let root = root()?;
    let Some(_guard) = lock(&root)? else {
        return Ok(());
    };
    let previous = admission(&root).filter(|a| a.fingerprint == input.fingerprint);
    // Two slow, successful reads of the identical inventory are the admission
    // signal. At most one attempt is made for that observed input.
    if previous.as_ref().is_some_and(|a| a.attempt.is_some()) {
        return Ok(());
    }
    let attempt = previous.map(|_| uuid::Uuid::new_v4());
    let value = serde_json::to_vec(&Admission {
        fingerprint: input.fingerprint.clone(),
        roots: roots.to_vec(),
        attempt: attempt.map(|id| id.to_string()),
    })?;
    if value.len() > MAX_ADMISSION {
        return Ok(());
    }
    crate::storage::atomic_write(&root.join(ADMISSION), &value)?;
    if let Some(attempt) = attempt {
        JOB.with(|job| *job.borrow_mut() = Some(attempt));
    }
    Ok(())
}

/// Called by the executable after it has flushed a successful foreground result.
pub fn start_queued() {
    let Some(ticket) = JOB.with(|job| job.borrow_mut().take()) else {
        return;
    };
    let Ok(binary) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(binary);
    command
        .arg("--usage-reuse-worker")
        .arg(ticket.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    platform::prepare_command(&mut command);
    // The worker has a one-shot deadline and its own lock, with no inherited
    // response pipe or foreground process-group dependency.
    let _ = command.spawn();
}

struct Budget<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> Write for Budget<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other("usage cache size limit"));
        }
        let n = self.inner.write(bytes)?;
        self.remaining -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn encode(facts: &Collected, fingerprint: &str) -> Result<Vec<u8>> {
    let header = format!("{SIGNATURE}:{fingerprint}\n").into_bytes();
    let remaining = MAX_ENCODED - header.len();
    let mut encoder = zstd::stream::write::Encoder::new(
        Budget {
            inner: header,
            remaining,
        },
        1,
    )?;
    encoder.include_checksum(true)?;
    let mut writer = Budget {
        inner: encoder,
        remaining: MAX_DECODED,
    };
    serde_json::to_writer(&mut writer, facts)?;
    Ok(writer.inner.finish()?.inner)
}

pub fn run_worker(ticket: &str) -> Result<()> {
    if !platform::SUPPORTED {
        return Ok(());
    }
    let ticket = uuid::Uuid::parse_str(ticket)?.to_string();
    let root = root()?;
    let Some(_guard) = lock(&root)? else {
        return Ok(());
    };
    let Some(job) = admission(&root).filter(|a| a.attempt.as_deref() == Some(ticket.as_str()))
    else {
        return Ok(());
    };
    platform::lower_priority();
    // Without a working memory probe the optional build cannot enforce its budget.
    if platform::peak_memory()? > MEMORY_LIMIT {
        return Ok(());
    }
    let context = RunContext {
        max_files: usage_input::MAX_FILES,
        max_bytes: usage_input::MAX_BYTES,
        ..RunContext::default()
    };
    let cancelled = Arc::clone(&context.cancelled);
    std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            if cancelled.load(Ordering::Relaxed)
                || started.elapsed() >= BUILD_LIMIT
                || !matches!(platform::peak_memory(), Ok(bytes) if bytes <= MEMORY_LIMIT)
            {
                cancelled.store(true, Ordering::Relaxed);
                std::thread::sleep(Duration::from_secs(2));
                std::process::exit(1);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let Some(before) = candidate(&job.roots, false).filter(|i| i.fingerprint == job.fingerprint)
    else {
        return Ok(());
    };
    // File metadata can block independently of the parser. Keep it off the
    // watchdog so a slow source cannot suspend the worker's deadline.
    let cancelled = Arc::clone(&context.cancelled);
    let watched_roots = job.roots.clone();
    let watched_input = before.fingerprint.clone();
    std::thread::spawn(move || {
        while !cancelled.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(2));
            if candidate(&watched_roots, false).is_none_or(|i| i.fingerprint != watched_input) {
                cancelled.store(true, Ordering::Relaxed);
                return;
            }
        }
    });
    let facts = super::usage_source::collect(&job.roots, false, &context)?;
    // Only commit a complete observation of the admitted file inventory. A
    // malformed record can still be a complete observation of its bytes.
    if facts
        .watermarks
        .iter()
        .any(|w| w.observed_bytes != Some(w.committed_offset))
        || facts
            .sources
            .iter()
            .any(|s| !matches!(s.status.as_str(), "complete" | "partial"))
    {
        return Ok(());
    }
    let encoded = encode(&facts, &before.fingerprint)?;
    if context.is_cancelled()
        || platform::peak_memory()? > MEMORY_LIMIT
        || candidate(&job.roots, false).is_none_or(|i| i.fingerprint != before.fingerprint)
    {
        return Ok(());
    }
    crate::storage::atomic_write(&root.join(FILE), &encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_writer_cannot_start_another_build() {
        let directory = tempfile::tempdir().unwrap();
        let held = lock(directory.path()).unwrap().unwrap();
        assert!(lock(directory.path()).unwrap().is_none());
        drop(held);
        assert!(lock(directory.path()).unwrap().is_some());
    }

    #[test]
    fn compressed_ledger_can_be_bound_to_one_source_inventory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(FILE);
        let current = "a".repeat(64);
        let other = "b".repeat(64);
        let facts = Collected::default();
        fs::write(&path, encode(&facts, &current).unwrap()).unwrap();
        assert!(load_file(&path, &current).is_some());
        assert!(load_file(&path, &other).is_none());
        let mut bad = fs::read(&path).unwrap();
        bad.pop();
        fs::write(&path, bad).unwrap();
        assert!(load_file(&path, &current).is_none());
    }
}
