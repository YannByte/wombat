use crate::{absolute, home};
use anyhow::Result;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn data_home() -> Result<PathBuf> {
    if let Some(dir) = crate::host_environment::get("WOMBAT_DATA_HOME") {
        return absolute(PathBuf::from(dir));
    }
    if cfg!(target_os = "macos") {
        return Ok(home().join("Library/Application Support/Wombat"));
    }
    if cfg!(windows) {
        return Ok(crate::host_environment::get("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData/Local"))
            .join("Wombat"));
    }
    Ok(crate::host_environment::get("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
        .join("wombat"))
}

pub fn atomic_write(file: &Path, content: &[u8]) -> Result<()> {
    atomic_with(file, |output| {
        output.write_all(content)?;
        Ok(())
    })
}
/// Create a private product file before a database opens it; reject existing links/non-files.
pub(crate) fn private_file(file: &Path) -> Result<()> {
    let parent = file
        .parent()
        .ok_or_else(|| crate::dto::operation_error("INVALID_ARGUMENT", "Missing data parent"))?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(parent)?;
    match fs::symlink_metadata(file) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(crate::dto::operation_error(
            "STORAGE_UNAVAILABLE",
            "Unsafe product file",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(file) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => private_file(file),
                Err(e) => Err(e.into()),
            }
        }
        Err(e) => Err(e.into()),
    }
}
/// Write a new file in a caller-owned unpublished generation. The caller must
/// sync that directory before publishing it; existing files are never replaced.
pub(crate) fn write_unpublished(file: &Path, content: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(file)?;
    output.write_all(content)?;
    output.sync_all()?;
    Ok(())
}
fn atomic_with(file: &Path, write: impl FnOnce(&mut fs::File) -> Result<()>) -> Result<()> {
    let file = absolute(file)?;
    let parent = file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("文件路径没有父目录"))?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    temp.persist(&file)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unpublished_files_are_private_and_cannot_replace_existing_targets() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shard.json");
        write_unpublished(&file, b"complete shard").unwrap();
        assert!(write_unpublished(&file, b"replacement").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"complete shard");
        #[cfg(unix)]
        {
            use std::os::unix::{fs::PermissionsExt, fs::symlink};
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = dir.path().join("link.json");
            symlink(&file, &link).unwrap();
            assert!(write_unpublished(&link, b"replacement").is_err());
            assert_eq!(fs::read(&file).unwrap(), b"complete shard");
        }
    }
    #[test]
    fn private_atomic_file() {
        let dir = std::env::temp_dir().join(format!("wombat-rust-{}", uuid::Uuid::new_v4()));
        let file = dir.join("snapshot.json");
        atomic_write(&file, b"first").unwrap();
        atomic_write(&file, b"second").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
