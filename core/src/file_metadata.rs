//! Capture file identity and attributes together; creation time is not identity.
use std::{fs, io, path::Path, time::SystemTime};

#[derive(Clone)]
pub(crate) struct FileMetadata {
    metadata: fs::Metadata,
    pub(crate) physical: String,
}

impl FileMetadata {
    pub(crate) fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        #[cfg(windows)]
        {
            // Metadata probes also observe a directory replacing a source file.
            let handle = winapi_util::Handle::from_path_any(path)?;
            Self::from_file(handle.as_file())
        }
        #[cfg(not(windows))]
        {
            Ok(Self::from_metadata(fs::metadata(path)?))
        }
    }

    pub(crate) fn from_file(file: &fs::File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        #[cfg(windows)]
        {
            // Safe maintained wrapper around GetFileInformationByHandle. Both
            // observations belong to this open handle, even during a rename.
            let info = winapi_util::file::information(file)?;
            Ok(Self {
                metadata,
                physical: format!("{}:{}", info.volume_serial_number(), info.file_index()),
            })
        }
        #[cfg(not(windows))]
        Ok(Self::from_metadata(metadata))
    }

    #[cfg(not(windows))]
    pub(crate) fn from_metadata(metadata: fs::Metadata) -> Self {
        #[cfg(unix)]
        let physical = {
            use std::os::unix::fs::MetadataExt;
            format!("{}:{}", metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let physical = format!("{:?}", metadata.created().ok());
        Self { metadata, physical }
    }

    pub(crate) fn len(&self) -> u64 {
        self.metadata.len()
    }

    pub(crate) fn is_file(&self) -> bool {
        self.metadata.is_file()
    }

    /// Capture the change time from the same file observation used for reuse.
    #[cfg(unix)]
    pub(crate) fn read_for_reuse(path: &Path) -> io::Result<(Self, (i64, i64))> {
        use std::os::unix::fs::MetadataExt;
        let meta = Self::read(path)?;
        let change = (meta.metadata.ctime(), meta.metadata.ctime_nsec());
        Ok((meta, change))
    }

    #[cfg(windows)]
    pub(crate) fn read_for_reuse(path: &Path) -> io::Result<(Self, (i64, i64))> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_BASIC_INFO, FileBasicInfo, GetFileInformationByHandleEx,
        };
        let handle = winapi_util::Handle::from_path_any(path)?;
        let file = handle.as_file();
        let meta = Self::from_file(file)?;
        let mut info = FILE_BASIC_INFO::default();
        // SAFETY: the live file handle and correctly sized buffer match FileBasicInfo.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileBasicInfo,
                (&mut info as *mut FILE_BASIC_INFO).cast(),
                std::mem::size_of::<FILE_BASIC_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        if info.ChangeTime <= 0 {
            return Err(io::ErrorKind::Unsupported.into());
        }
        Ok((meta, (info.ChangeTime, 0)))
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn read_for_reuse(_path: &Path) -> io::Result<(Self, (i64, i64))> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(crate) fn modified(&self) -> io::Result<SystemTime> {
        self.metadata.modified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::os::windows::fs::FileTimesExt;

    #[test]
    fn path_metadata_distinguishes_missing_directory_and_regular_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        assert!(FileMetadata::read(&path).is_err());
        fs::create_dir(&path).unwrap();
        let directory = FileMetadata::read(&path).unwrap();
        assert!(!directory.is_file());
        fs::remove_dir(&path).unwrap();
        fs::write(&path, b"source").unwrap();
        let file = FileMetadata::read(&path).unwrap();
        assert!(file.is_file());
    }

    #[test]
    fn same_bytes_and_times_do_not_hide_replacement_or_rebind_open_handles() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("log");
        let replacement = root.path().join("replacement");
        fs::write(&path, b"same bytes").unwrap();
        let original = fs::File::open(&path).unwrap();
        let before = FileMetadata::from_file(&original).unwrap();
        fs::write(&replacement, b"same bytes").unwrap();
        let times = fs::FileTimes::new().set_modified(before.modified().unwrap());
        #[cfg(windows)]
        let times = times.set_created(original.metadata().unwrap().created().unwrap());
        fs::OpenOptions::new()
            .write(true)
            .open(&replacement)
            .unwrap()
            .set_times(times)
            .unwrap();
        fs::rename(&replacement, &path).unwrap();
        let after = FileMetadata::read(&path).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        assert_ne!(before.physical, after.physical);
        assert_eq!(
            before.physical,
            FileMetadata::from_file(&original).unwrap().physical
        );
    }
}
