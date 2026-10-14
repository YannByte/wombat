//! Select cached or live revisions, coordinate refresh jobs and expose actual freshness.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Business entries validate their own parameters before requesting a read view.
/// Selection owns only source scope, revision identity and synchronization policy.
pub(super) struct ReadViewSelector<'a> {
    pub(super) peer_closed: Option<&'a (dyn Fn() -> bool + Sync)>,
    pub(super) wait_ms: Option<u64>,
    pub(super) rebuild: bool,
    pub(super) include_history: bool,
    roots: Vec<String>,
    selector: Option<String>,
    mode: Mode,
    verify: bool,
    refresh: bool,
    capture_now: bool,
    wait: WaitPolicy,
    key: String,
    project: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitPolicy {
    None,
    Auto,
    Fresh,
}
impl WaitPolicy {
    fn timeout(self) -> Duration {
        Duration::from_secs(match self {
            Self::None => 0,
            Self::Auto => 2,
            Self::Fresh => 10,
        })
    }
}
impl ReadViewSelector<'_> {
    pub(super) fn project(mut self, project: Option<String>) -> Self {
        self.project = project;
        self
    }
    pub(super) fn capture_now(mut self) -> Self {
        self.capture_now = true;
        self
    }
    pub(super) fn identity(&self) -> Option<&String> {
        self.selector.as_ref()
    }
    pub(super) fn new(
        roots: Vec<String>,
        selector: Option<String>,
        mode: Mode,
        verify: bool,
        refresh: bool,
    ) -> Result<Self> {
        if (verify && (mode == Mode::Cached || selector.is_some()))
            || (refresh && (mode == Mode::Cached || selector.is_some()))
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "实时同步选项与读取版本不匹配",
            ));
        }
        let key = if let Some(id) = &selector {
            let mut parts = id.split(':');
            let prefix = parts.next();
            let scope = parts.next();
            let revision = parts.next();
            if prefix != Some("live")
                || scope.is_none_or(str::is_empty)
                || revision.is_none_or(str::is_empty)
                || parts.next().is_some()
            {
                return Err(operation_error("INVALID_ARGUMENT", "实时读取版本身份无效"));
            }
            let key = scope.expect("validated scope").to_owned();
            // Omitted scope preserves access to an already published read identity.
            // Explicit source roots cannot silently select a different source set.
            if !roots.is_empty() && source_key(&roots) != key {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "来源范围与实时读取版本不匹配",
                ));
            }
            key
        } else {
            source_key(&roots)
        };
        let wait = if selector.is_some() || mode == Mode::Cached {
            WaitPolicy::None
        } else if refresh || mode == Mode::Fresh {
            WaitPolicy::Fresh
        } else {
            WaitPolicy::Auto
        };
        Ok(Self {
            peer_closed: None,
            wait_ms: None,
            rebuild: false,
            include_history: true,
            roots,
            selector,
            mode,
            verify,
            refresh,
            capture_now: false,
            wait,
            key,
            project: None,
        })
    }
}
fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        Err(operation_error("CANCELLED", "读取已取消"))
    } else {
        Ok(())
    }
}

pub(super) fn select_view(
    request: &ReadViewSelector,
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    cancelled: &AtomicBool,
) -> Result<(Arc<Snapshot>, Freshness)> {
    let check_reader = || {
        check_cancel(cancelled)?;
        if request.peer_closed.is_some_and(|probe| probe()) {
            return Err(operation_error("CANCELLED", "Caller disconnected"));
        }
        Ok(())
    };
    check_reader()?;
    let roots = request.roots.clone();
    let selector = &request.selector;
    let key = &request.key;
    let (lock, wake) = &**shared;
    let mut entries = lock.lock().unwrap();
    // A fixed lookup cannot establish synchronization scope. In particular,
    // omitted roots must not seed an entry later reused by an explicit-root read.
    if selector.is_some() && !entries.contains_key(key) {
        return Err(operation_error(
            "VIEW_EXPIRED",
            "这个实时读取版本已过期，请重新打开列表",
        ));
    }
    let entry = entries
        .entry(key.clone())
        .or_insert_with(|| Entry::new(roots));
    entry.touched = Instant::now();
    entry.preferred_project = request.project.clone();
    let history_requested = selector.is_none()
        && request.include_history
        && entry.views.back().is_some_and(|(_, v)| {
            v.manifest
                .issues
                .iter()
                .any(|i| i.code == "historyDeferred")
        });
    if selector.is_none() {
        entry.include_history |= request.include_history;
    }
    let mut ticket = entry.completed;
    if request.wait != WaitPolicy::None
        && (request.mode == Mode::Fresh
            || request.refresh
            || request.capture_now
            || entry.requested == 0
            || history_requested
            || entry.syncing
            || scheduling::auto_due(entry))
    {
        ticket = scheduling::request_work(entry, key, request.verify, request.rebuild, jobs)?;
        let preview_at = Instant::now() + Duration::from_millis(250);
        let deadline = Instant::now()
            + request
                .wait_ms
                .map(Duration::from_millis)
                .unwrap_or_else(|| request.wait.timeout());
        while entries[key].completed < ticket && Instant::now() < deadline {
            check_reader()?;
            if request.wait == WaitPolicy::Auto
                && !request.capture_now
                && entries[key].views.back().is_some_and(|(_, v)| {
                    !preview::is_initial(v) && !collection::restoring_projects(v)
                })
            {
                break;
            }
            let initial_available = request.wait == WaitPolicy::Auto
                && entries[key].views.back().is_some_and(|(_, v)| {
                    preview::is_initial(v) || collection::restoring_projects(v)
                });
            if initial_available && Instant::now() >= preview_at {
                break;
            }
            let wait = if initial_available {
                preview_at.min(deadline)
            } else {
                deadline
            }
            .saturating_duration_since(Instant::now());
            // Cancelling one reader never removes or interrupts shared work.
            entries = wake
                .wait_timeout(entries, wait.min(Duration::from_millis(50)))
                .unwrap()
                .0;
        }
    } else if selector.is_none() && entry.views.is_empty() {
        // Restoration has one worker owner. Readers never independently load the same index.
        if !entry.restoring && !entry.syncing {
            jobs.try_send(Job {
                key: key.clone(),
                restore_only: true,
            })
            .map_err(|_| operation_error("UPDATE_BUSY", "读取请求队列已满，请稍后重试"))?;
            entry.restoring = true;
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while entries[key].views.is_empty()
            && (entries[key].restoring || entries[key].syncing)
            && Instant::now() < deadline
        {
            check_reader()?;
            entries = wake
                .wait_timeout(entries, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }
    check_reader()?;
    let entry = entries.get_mut(key).unwrap();
    let current =
        entry.completed >= ticket && entry.error.is_none() && !entry.syncing && !entry.restoring;
    if (request.mode == Mode::Fresh || request.refresh) && !current && selector.is_none() {
        return Err(operation_error(
            if entry.error.is_some() {
                entry.error_code.unwrap_or("SOURCE_UNREADABLE")
            } else {
                "SYNC_TIMEOUT"
            },
            "未能在限定时间内完成同步，请重试或使用 --cached",
        ));
    }
    let snapshot = if let Some(selector) = selector {
        entry
            .views
            .iter()
            .find(|(_, v)| &v.manifest.snapshot_ref.snapshot_id == selector)
            .map(|(_, v)| Arc::clone(v))
            .ok_or_else(|| {
                operation_error("VIEW_EXPIRED", "这个实时读取版本已过期，请重新打开列表")
            })?
    } else {
        entry
            .views
            .iter()
            .rev()
            .find(|(_, v)| request.mode != Mode::Cached || !preview::is_initial(v))
            .map(|(_, v)| Arc::clone(v))
            .ok_or_else(|| {
                operation_error(
                    if entry.error.is_some() {
                        entry.error_code.unwrap_or("SOURCE_UNREADABLE")
                    } else if request.mode == Mode::Cached && !entry.restoring {
                        "NO_SNAPSHOT"
                    } else {
                        "SYNC_PENDING"
                    },
                    if let Some(error) = &entry.error {
                        error.clone()
                    } else if entry.restoring {
                        "正在加载已保存的项目，请稍后重试".into()
                    } else if request.mode == Mode::Cached {
                        "尚无可读取的已提交索引，请先同步".into()
                    } else {
                        entry
                            .error
                            .clone()
                            .unwrap_or_else(|| "尚无已提交数据，同步正在进行，请稍后重试".into())
                    },
                )
            })?
    };
    let freshness = Freshness {
        publication_change: snapshot.publication_change.clone(),
        project_loads: snapshot.project_loads.clone(),
        initial_scan: preview::is_initial(&snapshot),
        status: if selector.is_some() {
            "fixed"
        } else if current {
            "current"
        } else if entry.error.is_some() {
            "failed"
        } else if entry.syncing || entry.restoring {
            "syncing"
        } else {
            "stale"
        }
        .into(),
        checked_at: entry.checked.clone(),
        revision: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
        error: entry.error.clone(),
        error_code: entry.error_code.map(String::from),
    };
    drop(entries);
    Ok((snapshot, freshness))
}

#[cfg(test)]
mod tests;
