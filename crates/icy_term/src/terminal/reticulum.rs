//! Reticulum remote shell (rnsh) transport: bridges icy_net's [`Connection`]
//! onto rsReticulum's rnsh client, merging remote stdout and stderr into one
//! read stream.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use icy_net::{Connection, ConnectionState, ConnectionType, NetError};
use rns_identity::identity::Identity;
use rns_runtime::lifecycle::ShutdownSignal;
use rns_runtime::reticulum::{self, ReticulumHandle};
use rns_runtime::rnsh::{rnsh_client_execute, RnshClientConfig, RnshWindowSize};
use tokio::sync::{mpsc, oneshot, OnceCell};

/// rnsh's `timeout` is a hard session cap, sized for one-shot commands; an
/// interactive BBS session needs effectively none.
const SESSION_MAX: Duration = Duration::from_secs(30 * 24 * 60 * 60); // ~30 days
const PATH_TIMEOUT: Duration = Duration::from_secs(20);
const CHANNEL_CAP: usize = 256;

/// Shared per process: `init` binds the transport and interfaces from the
/// user's existing Reticulum config, so one instance serves every connection.
static RUNTIME: OnceCell<ReticulumHandle> = OnceCell::const_new();

async fn shared_runtime() -> icy_net::Result<&'static ReticulumHandle> {
    RUNTIME
        .get_or_try_init(|| async {
            let shutdown = ShutdownSignal::new();
            let is_foreground = Arc::new(AtomicBool::new(true));
            // None => rsReticulum's default config dir, shared with rnsd.
            reticulum::init(None, None, shutdown, is_foreground).await
        })
        .await
        .map_err(|e| boxed(format!("Reticulum init failed: {e}")))
}

fn boxed(msg: String) -> Box<dyn std::error::Error + Send + Sync> {
    msg.into()
}

fn identity_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "GitHub", "icy_term").map(|d| d.config_dir().join("reticulum_identity"))
}

/// Persistent client identity; listeners authenticate against it (`rnsh-rs -a`).
fn load_identity() -> Identity {
    let path = identity_path();
    if let Some(path) = &path {
        if path.exists() {
            match Identity::from_file(path) {
                Ok(id) => return id,
                Err(e) => log::warn!("Could not read Reticulum identity, creating a new one: {e}"),
            }
        }
    }
    let id = Identity::new();
    if let Some(path) = &path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = id.to_file(path) {
            log::warn!("Could not persist Reticulum identity: {e}");
        }
    }
    id
}

/// Parse a Reticulum destination hash (16 bytes / 32 hex chars), tolerating an
/// optional `rns://` prefix and trailing slash.
pub fn parse_destination(s: &str) -> Option<[u8; 16]> {
    let s = s.trim().trim_start_matches("rns://").trim_end_matches('/').trim();
    if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

pub struct ReticulumConnection {
    stdin_tx: mpsc::Sender<Vec<u8>>,
    stdout_rx: mpsc::Receiver<Vec<u8>>,
    window_tx: mpsc::Sender<RnshWindowSize>,
    read_buffer: Vec<u8>,
    /// Cancels the rnsh session; sending (or dropping) ends the session future.
    cancel: Option<oneshot::Sender<()>>,
    closed: bool,
}

impl ReticulumConnection {
    /// Interactive rnsh session; `cols`/`rows` seed the remote PTY.
    pub async fn open(destination_hash: [u8; 16], cols: u16, rows: u16) -> icy_net::Result<Self> {
        let handle = shared_runtime().await?;

        // Fail fast on an unreachable destination instead of blocking inside the
        // rnsh client for the whole (very large) session timeout.
        handle
            .await_path(destination_hash, PATH_TIMEOUT)
            .await
            .map_err(|e| boxed(format!("No path to Reticulum destination: {e}")))?;

        let identity = load_identity();

        let (stdin_tx, stdin_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_CAP);
        let (out_tx, stdout_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_CAP);
        let (window_tx, window_rx) = mpsc::channel::<RnshWindowSize>(8);

        let cfg = RnshClientConfig {
            identity,
            destination_hash,
            command: Vec::new(),
            no_id: false,
            timeout: SESSION_MAX,
            stdin_data: Vec::new(),
            stdin_rx: Some(stdin_rx),
            stdout_tx: Some(out_tx.clone()),
            stderr_tx: Some(out_tx),
            window_rx: Some(window_rx),
            pipe_stdin: false,
            pipe_stdout: false,
            pipe_stderr: false,
            term: Some("xterm-256color".to_string()),
            rows: Some(rows as u32),
            cols: Some(cols as u32),
            hpix: None,
            vpix: None,
        };

        let transport_tx = handle.transport_tx.clone();
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

        // `rnsh_client_execute`'s future is `!Send` (holds `dyn MessageBase`
        // across awaits), so it needs its own current-thread runtime, not spawn.
        std::thread::Builder::new()
            .name("rnsh-session".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        log::error!("Could not start rnsh session runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    tokio::select! {
                        result = rnsh_client_execute(transport_tx, cfg) => match result {
                            Ok(outcome) => log::info!("rnsh session ended (return code {})", outcome.return_code),
                            Err(e) => log::info!("rnsh session ended: {e}"),
                        },
                        _ = cancel_rx => log::debug!("rnsh session cancelled"),
                    }
                });
            })
            .map_err(|e| boxed(format!("Could not spawn rnsh session thread: {e}")))?;

        Ok(Self {
            stdin_tx,
            stdout_rx,
            window_tx,
            read_buffer: Vec::new(),
            cancel: Some(cancel_tx),
            closed: false,
        })
    }

    /// Resize the remote PTY. Unused: `terminal_thread` boxes this as
    /// `dyn Connection`, so the PTY keeps the size `open` seeded.
    pub async fn resize(&self, cols: u16, rows: u16) {
        let _ = self
            .window_tx
            .send(RnshWindowSize {
                rows: Some(rows as u32),
                cols: Some(cols as u32),
                hpix: None,
                vpix: None,
            })
            .await;
    }

    fn take_from_buffer(&mut self, buf: &mut [u8]) -> usize {
        let n = buf.len().min(self.read_buffer.len());
        buf[..n].copy_from_slice(&self.read_buffer[..n]);
        self.read_buffer.drain(..n);
        n
    }
}

#[async_trait]
impl Connection for ReticulumConnection {
    fn get_connection_type(&self) -> ConnectionType {
        ConnectionType::Reticulum
    }

    async fn read(&mut self, buf: &mut [u8]) -> icy_net::Result<usize> {
        if !self.read_buffer.is_empty() {
            return Ok(self.take_from_buffer(buf));
        }
        match self.stdout_rx.recv().await {
            Some(chunk) => {
                self.read_buffer = chunk;
                Ok(self.take_from_buffer(buf))
            }
            None => {
                self.closed = true;
                Ok(0)
            }
        }
    }

    async fn try_read(&mut self, buf: &mut [u8]) -> icy_net::Result<usize> {
        if !self.read_buffer.is_empty() {
            return Ok(self.take_from_buffer(buf));
        }
        match self.stdout_rx.try_recv() {
            Ok(chunk) => {
                self.read_buffer = chunk;
                Ok(self.take_from_buffer(buf))
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(0),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.closed = true;
                Ok(0)
            }
        }
    }

    async fn send(&mut self, buf: &[u8]) -> icy_net::Result<()> {
        self.stdin_tx
            .send(buf.to_vec())
            .await
            .map_err(|_| boxed(NetError::ConnectionClosed.to_string()))
    }

    async fn poll(&mut self) -> icy_net::Result<ConnectionState> {
        // Stay Connected until buffered output is drained.
        if self.closed && self.read_buffer.is_empty() {
            Ok(ConnectionState::Disconnected)
        } else {
            Ok(ConnectionState::Connected)
        }
    }

    async fn shutdown(&mut self) -> icy_net::Result<()> {
        self.closed = true;
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        Ok(())
    }
}

impl Drop for ReticulumConnection {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_destination;

    #[test]
    fn parses_bare_hash() {
        let hash = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
        let parsed = parse_destination(hash).expect("valid hash");
        assert_eq!(parsed[0], 0xa1);
        assert_eq!(parsed[15], 0x90);
    }

    #[test]
    fn parses_with_scheme_and_slash() {
        let a = parse_destination("rns://a1b2c3d4e5f60718293a4b5c6d7e8f90/").unwrap();
        let b = parse_destination("A1B2C3D4E5F60718293A4B5C6D7E8F90").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_bad_length_and_nonhex() {
        assert!(parse_destination("deadbeef").is_none());
        assert!(parse_destination("a1b2c3d4e5f60718293a4b5c6d7e8f9").is_none()); // 31 chars
        assert!(parse_destination("a1b2c3d4e5f60718293a4b5c6d7e8f900").is_none()); // 33 chars
        assert!(parse_destination("g1b2c3d4e5f60718293a4b5c6d7e8f90").is_none());
    }
}
