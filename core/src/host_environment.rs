//! Foreground children inherit the invoking CLI environment.
pub(crate) fn get(key: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(key)
}
