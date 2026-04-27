//! IPC server — listens on a Unix domain socket and forwards requests to the
//! TUI event loop via an `mpsc` channel.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use super::protocol::{IpcRequest, IpcResponse};

/// A message from the IPC server to the TUI event loop.
///
/// The TUI processes the request, builds a response, and sends it back
/// through `response_tx`.
pub struct IpcMessage {
    pub request: IpcRequest,
    pub response_tx: mpsc::Sender<IpcResponse>,
}

/// Start the IPC server in a background thread.
///
/// Returns a receiver for incoming IPC messages. On platforms without
/// Unix socket support, returns an empty receiver (IPC is a no-op).
pub fn start_ipc_server(stop: Arc<AtomicBool>) -> std::io::Result<mpsc::Receiver<IpcMessage>> {
    #[cfg(unix)]
    {
        start_unix_server(stop)
    }
    #[cfg(not(unix))]
    {
        let _ = stop;
        let (_tx, rx) = mpsc::channel();
        Ok(rx)
    }
}

#[cfg(unix)]
fn start_unix_server(stop: Arc<AtomicBool>) -> std::io::Result<mpsc::Receiver<IpcMessage>> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();
    let path = super::protocol::socket_path();

    // Remove stale socket file from a previous run
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;

    std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

                        let mut reader = BufReader::new(&stream);
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() {
                            continue;
                        }

                        let request = match serde_json::from_str::<IpcRequest>(line.trim()) {
                            Ok(req) => req,
                            Err(e) => {
                                let resp = IpcResponse::error(format!("Invalid JSON: {e}"));
                                if let Ok(json) = serde_json::to_string(&resp) {
                                    let _ = writeln!(&stream, "{json}");
                                }
                                continue;
                            }
                        };

                        let (resp_tx, resp_rx) = mpsc::channel();
                        let msg = IpcMessage {
                            request,
                            response_tx: resp_tx,
                        };

                        if tx.send(msg).is_err() {
                            break; // TUI has closed
                        }

                        let response = resp_rx
                            .recv_timeout(Duration::from_secs(5))
                            .unwrap_or_else(|_| IpcResponse::error("Timeout waiting for response"));

                        if let Ok(json) = serde_json::to_string(&response) {
                            let _ = writeln!(&stream, "{json}");
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            // Cleanup socket file
            let _ = std::fs::remove_file(super::protocol::socket_path());
        })?;

    Ok(rx)
}
