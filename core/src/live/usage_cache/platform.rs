//! Native worker setup and process memory observations, reported in bytes.
use std::{io, process::Command};

pub(super) const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "linux", windows));

pub(super) fn prepare_command(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{
            BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW,
        };
        command.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
    }
}

pub(super) fn lower_priority() {
    #[cfg(target_os = "macos")]
    // SAFETY: set only the calling thread's supported scheduling class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(target_os = "linux")]
    super::super::resources::background_indexing();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn peak_memory() -> io::Result<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the structure on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let peak = unsafe { usage.assume_init() }.ru_maxrss.max(0) as u64;
    // Linux reports KiB, while macOS reports bytes.
    #[cfg(target_os = "linux")]
    let peak = peak.saturating_mul(1024);
    Ok(peak)
}

#[cfg(windows)]
pub(super) fn peak_memory() -> io::Result<u64> {
    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let mut info = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..Default::default()
    };
    // SAFETY: the current process pseudo-handle and sized output buffer are valid.
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut info, size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.PeakWorkingSetSize as u64)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub(super) fn peak_memory() -> io::Result<u64> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests {
    #[test]
    fn worker_can_observe_nonzero_peak_memory_in_bytes() {
        assert!(super::peak_memory().unwrap() >= 1024);
    }
}
