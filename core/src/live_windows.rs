//! Owner-only local named pipes backed by Windows overlapped I/O.
use interprocess::os::windows::security_descriptor::{AsSecurityDescriptor, SecurityDescriptor};
use std::{
    cell::Cell,
    ffi::c_void,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    runtime::Runtime,
    time::timeout,
};
use widestring::u16cstr;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;

fn trace(message: impl AsRef<str>) {
    if std::env::var_os("WOMBAT_SERVICE_TRACE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        eprintln!("[wombat-service] {}", message.as_ref());
    }
}

pub struct Listener {
    runtime: Arc<Runtime>,
    incoming: Mutex<mpsc::Receiver<io::Result<NamedPipeServer>>>,
}
pub struct Stream {
    runtime: Arc<Runtime>,
    pipe: Mutex<NamedPipeServer>,
    read_deadline: Cell<Instant>,
    write_deadline: Cell<Instant>,
    sent: bool,
}

fn create_pipe(
    path: &Path,
    first: bool,
    security_descriptor: &SecurityDescriptor,
) -> io::Result<NamedPipeServer> {
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security_descriptor.as_sd().cast_mut(),
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .access_inbound(true)
        .access_outbound(true)
        // Eight active clients, eight queued connections and one connection/pending pair.
        .max_instances(18)
        .in_buffer_size(65_536)
        .out_buffer_size(65_536);
    // SAFETY: attributes points to a valid, non-inheritable SECURITY_ATTRIBUTES value and
    // the owned descriptor remains alive for the complete CreateNamedPipeW call.
    unsafe {
        options.create_with_security_attributes_raw(
            path.as_os_str(),
            (&raw mut attributes).cast::<c_void>(),
        )
    }
}

impl Listener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_io()
                .enable_time()
                .build()?,
        );
        // Protected DACL: only the creator/owner. Remote pipe clients are also rejected.
        let security_descriptor = Arc::new(SecurityDescriptor::deserialize(u16cstr!(
            "D:P(A;;GA;;;OW)"
        ))?);
        let first = {
            let _runtime = runtime.enter();
            create_pipe(path, true, &security_descriptor)?
        };
        let path = PathBuf::from(path);
        let (sender, incoming) = mpsc::sync_channel(8);
        runtime.spawn(async move {
            let mut pipe = first;
            loop {
                if let Err(error) = pipe.connect().await {
                    trace(format!("client connection failed: {error:?}"));
                    let _ = sender.try_send(Err(error));
                    break;
                }
                trace("client connected on accept task");
                // Create the next instance before handing this one to the service so clients
                // never observe a gap with no listening pipe instance.
                let next = match create_pipe(&path, false, &security_descriptor) {
                    Ok(next) => {
                        trace("next listener instance created");
                        next
                    }
                    Err(error) => {
                        trace(format!("next listener creation failed: {error:?}"));
                        let _ = sender.try_send(Err(error));
                        break;
                    }
                };
                match sender.try_send(Ok(pipe)) {
                    Ok(()) | Err(mpsc::TrySendError::Full(_)) => pipe = next,
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        trace("accept task stopped after receiver closed");
                        break;
                    }
                }
            }
        });
        trace("listener bound");
        Ok(Self {
            runtime,
            incoming: Mutex::new(incoming),
        })
    }

    pub fn set_nonblocking(&self, _value: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn accept(&self) -> io::Result<(Stream, ())> {
        match self.incoming.lock().unwrap().try_recv() {
            Ok(Ok(pipe)) => {
                trace("client accepted");
                let deadline = Instant::now() + Duration::from_secs(12);
                Ok((
                    Stream {
                        runtime: Arc::clone(&self.runtime),
                        pipe: Mutex::new(pipe),
                        read_deadline: Cell::new(deadline),
                        write_deadline: Cell::new(deadline),
                        sent: false,
                    },
                    (),
                ))
            }
            Ok(Err(error)) => Err(error),
            Err(mpsc::TryRecvError::Empty) => Err(io::ErrorKind::WouldBlock.into()),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| io::ErrorKind::TimedOut.into())
}

impl Stream {
    pub fn peer_probe(&self) -> impl Fn() -> bool + Sync + '_ {
        let pipe = &self.pipe;
        move || {
            let mut byte = [0_u8; 1];
            match pipe.lock().unwrap().try_read(&mut byte) {
                Ok(0) => true,
                Err(error) => matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                ),
                Ok(_) => byte[0] == 0,
            }
        }
    }
    pub fn set_nonblocking(&self, _value: bool) -> io::Result<()> {
        Ok(())
    }
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.read_deadline
            .set(Instant::now() + timeout.unwrap_or(Duration::from_secs(12)));
        Ok(())
    }
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.write_deadline
            .set(Instant::now() + timeout.unwrap_or(Duration::from_secs(12)));
        Ok(())
    }
}

impl Read for &Stream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let duration = remaining(self.read_deadline.get())?;
        let mut pipe = self.pipe.lock().unwrap();
        let result = self
            .runtime
            .block_on(async { timeout(duration, pipe.read(output)).await })
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        trace(format!("read result: {result:?}"));
        result
    }
}

impl Write for Stream {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        trace(format!("writing {} byte(s)", input.len()));
        let duration = remaining(self.write_deadline.get())?;
        let mut pipe = self.pipe.lock().unwrap();
        let result = self
            .runtime
            .block_on(async { timeout(duration, pipe.write(input)).await })
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        trace(format!("write result: {result:?}"));
        if let Ok(count) = result {
            self.sent |= count > 0;
        }
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        let duration = remaining(self.write_deadline.get())?;
        let mut pipe = self.pipe.lock().unwrap();
        self.runtime
            .block_on(async { timeout(duration, pipe.flush()).await })
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if self.sent {
            // The client closes after reading the newline-delimited response. Waiting for that
            // acknowledgement preserves buffered data without allowing a stalled client to keep
            // the service blocked indefinitely.
            self.read_deadline.set(self.write_deadline.get());
            let acknowledgement = match (&*self).read(&mut [0; 1]) {
                Ok(0) => Ok(()),
                Ok(_) => Err(io::Error::from(io::ErrorKind::InvalidData)),
                Err(error) => Err(error),
            };
            trace(format!("client close acknowledgement: {acknowledgement:?}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use interprocess::os::windows::named_pipe::{DuplexPipeStream, pipe_mode::Bytes};

    #[test]
    fn expired_deadline_is_reported() {
        assert_eq!(
            remaining(Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }

    #[test]
    fn owner_can_exchange_large_response_and_idle_reads_expire() {
        let name = format!(r"\\.\pipe\wombat-test-{}", uuid::Uuid::new_v4());
        let listener = Listener::bind(Path::new(&name)).unwrap();
        assert!(
            Listener::bind(Path::new(&name)).is_err(),
            "first instance is exclusive"
        );
        let (idle_tx, idle_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, ()) = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_millis(30)))
                .unwrap();
            assert_eq!(
                (&stream).read(&mut [0; 1]).unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
            idle_tx.send(()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4];
            (&stream).read_exact(&mut request).unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(&vec![b'x'; 65_536]).unwrap();
        });
        let mut client = DuplexPipeStream::<Bytes>::connect_by_path(name.as_str()).unwrap();
        idle_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        client.write_all(b"ping").unwrap();
        let mut response = vec![0; 65_536];
        client.read_exact(&mut response).unwrap();
        assert!(response.iter().all(|byte| *byte == b'x'));
        drop(client);
        worker.join().unwrap();
    }

    #[test]
    fn closing_a_stalled_client_has_a_deadline() {
        let name = format!(r"\\.\pipe\wombat-test-{}", uuid::Uuid::new_v4());
        let listener = Listener::bind(Path::new(&name)).unwrap();
        let client = DuplexPipeStream::<Bytes>::connect_by_path(name.as_str()).unwrap();
        let (mut stream, ()) = loop {
            match listener.accept() {
                Ok(stream) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => panic!("accept failed: {error}"),
            }
        };
        stream
            .set_write_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        stream.write_all(b"response\n").unwrap();
        let start = Instant::now();
        drop(stream);
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(client);
    }

    #[test]
    fn response_survives_until_the_client_reads_it() {
        let name = format!(r"\\.\pipe\wombat-test-{}", uuid::Uuid::new_v4());
        let listener = Listener::bind(Path::new(&name)).unwrap();
        let mut client = DuplexPipeStream::<Bytes>::connect_by_path(name.as_str()).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, ()) = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream.write_all(b"response\n").unwrap();
        });
        std::thread::sleep(Duration::from_millis(50));
        let mut response = [0; 9];
        client.read_exact(&mut response).unwrap();
        assert_eq!(&response, b"response\n");
        drop(client);
        worker.join().unwrap();
    }
}
