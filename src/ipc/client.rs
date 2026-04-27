//! IPC client — connects to a running kora instance and sends a command.

use anyhow::Result;

use super::protocol::IpcResponse;

/// Send a command to a running kora instance via IPC and return the response.
#[cfg(unix)]
pub fn send_command(request: &super::protocol::IpcRequest) -> Result<IpcResponse> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use anyhow::Context;

    let path = super::protocol::socket_path();
    let stream = UnixStream::connect(&path).with_context(|| {
        format!(
            "Cannot connect to kora at {} — is kora running?",
            path.display()
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let json = serde_json::to_string(request).context("Failed to serialize request")?;
    writeln!(&stream, "{json}").context("Failed to send request")?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("Failed to read response")?;

    let response: IpcResponse =
        serde_json::from_str(line.trim()).context("Failed to parse response")?;
    Ok(response)
}

/// IPC is not yet supported on non-Unix platforms.
#[cfg(not(unix))]
pub fn send_command(_request: &super::protocol::IpcRequest) -> Result<IpcResponse> {
    anyhow::bail!("IPC remote control is not yet supported on this platform")
}
