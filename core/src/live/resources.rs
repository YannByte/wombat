//! Keep interactive reads independent from background indexing and idle heaps.
pub(super) fn background_indexing() {
    // These are scheduling hints, not CPU or memory quotas.
    #[cfg(target_os = "macos")]
    unsafe {
        // SAFETY: changes only the calling worker's supported QoS class.
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_BACKGROUND, 0);
    }
    #[cfg(target_os = "linux")]
    unsafe {
        // SAFETY: Linux accepts a native thread ID for PRIO_PROCESS. Increasing
        // nice lowers this worker's priority without affecting request threads.
        let id = libc::syscall(libc::SYS_gettid) as libc::id_t;
        if libc::getpriority(libc::PRIO_PROCESS, id) < 10 {
            libc::setpriority(libc::PRIO_PROCESS, id, 10);
        }
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };
        // SAFETY: the pseudo-handle denotes only this indexing thread.
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

/// A batch retires its thread-local allocator state before query headers are built.
pub(super) fn index_batch<T: Send>(
    work: impl FnOnce() -> anyhow::Result<T> + Send,
) -> anyhow::Result<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                background_indexing();
                work()
            })
            .join()
            .map_err(|_| {
                crate::dto::operation_error("CORE_UNAVAILABLE", "Indexing batch panicked")
            })?
    })
}

/// Recovery state is persisted in the transaction before this boundary. Bound its cache
/// before ledger aggregation and view publication can overlap with parser heaps.
pub(super) fn trim_parser_caches(
    db: &rusqlite::Connection,
    caches: &mut std::collections::BTreeMap<String, crate::adapters::codex::incremental::Cache>,
) -> anyhow::Result<(u64, bool)> {
    for (source, cache) in caches.iter_mut() {
        cache.refresh_encoded_bytes(db, source)?;
    }
    let mut bytes = caches
        .values()
        .map(|cache| cache.encoded_bytes)
        .sum::<u64>();
    let mut evicted = false;
    while bytes > super::service::PARSER_CACHE_BUDGET {
        let Some(key) = caches
            .iter()
            .max_by_key(|(_, cache)| cache.encoded_bytes)
            .map(|(id, _)| id.clone())
        else {
            break;
        };
        bytes = bytes.saturating_sub(caches.remove(&key).unwrap().encoded_bytes);
        evicted = true;
    }
    Ok((bytes, evicted))
}

pub(super) fn interactive_request() {
    #[cfg(target_os = "macos")]
    unsafe {
        // SAFETY: this changes only the current request thread's QoS.
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
    }
}

pub(super) fn release_unused() {
    // SAFETY: collection purges only unused mimalloc pages. Retained objects,
    // including values shared with readers, remain valid.
    unsafe { libmimalloc_sys::mi_collect(true) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn index_batch_retirement_preserves_returned_values_and_failures() {
        let retained = super::index_batch(|| Ok(vec![19_u8; 1024 * 1024])).unwrap();
        assert!(retained.iter().all(|v| *v == 19));
        let error = super::index_batch::<()>(|| {
            Err(crate::dto::operation_error(
                "INDEX_UNAVAILABLE",
                "synthetic failure",
            ))
        })
        .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "INDEX_UNAVAILABLE"
        );
        assert_eq!(super::index_batch(|| Ok(7)).unwrap(), 7);
    }

    #[test]
    fn background_hint_is_local_to_the_worker_and_relief_keeps_live_values() {
        crate::configure_allocator();
        // Keep the hand-bound option tied to the locked C header.
        assert_eq!(unsafe { libmimalloc_sys::mi_version() }, 20302);
        assert_eq!(unsafe { libmimalloc_sys::mi_option_get(27) }, 1);
        let retained = vec![17_u8; 1024 * 1024];
        std::thread::spawn(|| {
            super::background_indexing();
            #[cfg(target_os = "macos")]
            unsafe {
                let (mut class, mut priority) = (libc::qos_class_t::QOS_CLASS_UNSPECIFIED, 0);
                assert_eq!(
                    libc::pthread_get_qos_class_np(libc::pthread_self(), &mut class, &mut priority),
                    0
                );
                assert_eq!(class as u32, libc::qos_class_t::QOS_CLASS_BACKGROUND as u32);
            }
            #[cfg(target_os = "linux")]
            unsafe {
                assert!(
                    libc::getpriority(
                        libc::PRIO_PROCESS,
                        libc::syscall(libc::SYS_gettid) as libc::id_t
                    ) >= 10
                );
            }
            super::interactive_request();
            #[cfg(target_os = "macos")]
            unsafe {
                let (mut class, mut priority) = (libc::qos_class_t::QOS_CLASS_UNSPECIFIED, 0);
                assert_eq!(
                    libc::pthread_get_qos_class_np(libc::pthread_self(), &mut class, &mut priority),
                    0
                );
                assert_eq!(
                    class as u32,
                    libc::qos_class_t::QOS_CLASS_USER_INITIATED as u32
                );
            }
            super::release_unused();
        })
        .join()
        .unwrap();
        assert!(retained.iter().all(|byte| *byte == 17));
    }
}
