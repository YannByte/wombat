//! Bounded read-only native probes. No shell, inherited Git routing, or output pipes.
use crate::dto::operation_error;
use anyhow::Result;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub(crate) struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub(crate) fn probe(
    program: &str,
    args: &[&std::ffi::OsStr],
    cancelled: &dyn Fn() -> bool,
    deadline: Instant,
) -> Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let stop = |child: &mut std::process::Child| {
        #[cfg(unix)]
        {
            // SAFETY: the child owns a new process group, still unreaped here.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    };
    let result = loop {
        let check = (|| -> Result<Option<std::process::ExitStatus>> {
            if cancelled() {
                return Err(operation_error("CANCELLED", "Artifact probe cancelled"));
            }
            if Instant::now() >= deadline {
                return Err(operation_error(
                    "TIMEOUT",
                    "Artifact probe deadline exceeded",
                ));
            }
            if stdout.metadata()?.len() + stderr.metadata()?.len() > 1024 * 1024 {
                return Err(operation_error(
                    "RESOURCE_LIMIT",
                    "Artifact probe output exceeded its budget",
                ));
            }
            Ok(child.try_wait()?)
        })();
        match check {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                stop(&mut child);
                break Err(error);
            }
        }
    }?;
    if stdout.metadata()?.len() + stderr.metadata()?.len() > 1024 * 1024 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Artifact probe output exceeded its budget",
        ));
    }
    let read = |file: &mut fs::File| -> Result<Vec<u8>> {
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = vec![];
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Artifact probe output exceeded its budget",
            ));
        }
        Ok(bytes)
    };
    Ok(Output {
        code: result.code(),
        stdout: read(&mut stdout)?,
        stderr: read(&mut stderr)?,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    #[test]
    fn probes_bound_output_deadlines_and_cancellation() {
        let deadline = || Instant::now() + Duration::from_secs(2);
        let value = probe(
            "/usr/bin/printf",
            &[OsStr::new("synthetic")],
            &|| false,
            deadline(),
        )
        .unwrap();
        assert_eq!(value.code, Some(0));
        assert_eq!(value.stdout, b"synthetic");
        let error = probe(
            "/bin/sleep",
            &[OsStr::new("5")],
            &|| false,
            Instant::now() + Duration::from_millis(25),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("deadline"));
        assert!(probe("/bin/sleep", &[OsStr::new("5")], &|| true, deadline()).is_err());
        assert!(probe("/usr/bin/yes", &[], &|| false, deadline()).is_err());
    }
}
