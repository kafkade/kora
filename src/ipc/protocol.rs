//! IPC protocol types — request, response, and player status.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A command sent from an IPC client to the running kora instance.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum IpcRequest {
    #[serde(rename = "play")]
    Play,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "toggle")]
    Toggle,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "next")]
    Next,
    #[serde(rename = "prev")]
    Prev,
    #[serde(rename = "volume")]
    Volume { db: f32 },
    #[serde(rename = "status")]
    Status,
}

/// Response sent back to the IPC client.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlayerStatus>,
}

/// Snapshot of the player state, returned by the `status` command.
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerStatus {
    pub state: String,
    pub track: Option<String>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume_db: f32,
    pub queue_position: usize,
    pub queue_total: usize,
}

impl IpcResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            status: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            status: None,
        }
    }

    pub fn with_status(status: PlayerStatus) -> Self {
        Self {
            ok: true,
            error: None,
            status: Some(status),
        }
    }
}

/// Return the IPC socket path for the current platform.
#[allow(dead_code)] // Used by server/client on Unix; Windows named pipe support deferred
pub fn socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir).join("kora.sock");
        }
        PathBuf::from("/tmp/kora.sock")
    }
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\kora")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_request_round_trip() {
        let cases: Vec<IpcRequest> = vec![
            IpcRequest::Play,
            IpcRequest::Pause,
            IpcRequest::Toggle,
            IpcRequest::Stop,
            IpcRequest::Next,
            IpcRequest::Prev,
            IpcRequest::Volume { db: -3.5 },
            IpcRequest::Status,
        ];
        for req in &cases {
            let json = serde_json::to_string(req).unwrap();
            let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
            // Verify the round-trip produces the same JSON
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn ipc_request_tagged_format() {
        let json = serde_json::to_string(&IpcRequest::Play).unwrap();
        assert_eq!(json, r#"{"command":"play"}"#);

        let json = serde_json::to_string(&IpcRequest::Volume { db: -2.0 }).unwrap();
        assert!(json.contains(r#""command":"volume""#));
        assert!(json.contains(r#""db":-2.0"#));
    }

    #[test]
    fn ipc_response_ok_serialization() {
        let resp = IpcResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""ok":true"#));
        assert!(!json.contains("error"));
        assert!(!json.contains("status"));
    }

    #[test]
    fn ipc_response_error_serialization() {
        let resp = IpcResponse::error("something broke");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""ok":false"#));
        assert!(json.contains("something broke"));
    }

    #[test]
    fn ipc_response_with_status_serialization() {
        let status = PlayerStatus {
            state: "playing".to_string(),
            track: Some("Artist — Title".to_string()),
            position_secs: 42.5,
            duration_secs: 180.0,
            volume_db: -3.0,
            queue_position: 2,
            queue_total: 10,
        };
        let resp = IpcResponse::with_status(status);
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert!(parsed.status.is_some());
        let s = parsed.status.unwrap();
        assert_eq!(s.state, "playing");
        assert_eq!(s.track.as_deref(), Some("Artist — Title"));
        assert!((s.position_secs - 42.5).abs() < f64::EPSILON);
        assert_eq!(s.queue_position, 2);
    }

    #[test]
    fn player_status_none_track() {
        let status = PlayerStatus {
            state: "stopped".to_string(),
            track: None,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume_db: 0.0,
            queue_position: 0,
            queue_total: 0,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: PlayerStatus = serde_json::from_str(&json).unwrap();
        assert!(parsed.track.is_none());
    }

    #[test]
    fn socket_path_is_non_empty() {
        let path = socket_path();
        assert!(!path.as_os_str().is_empty());
    }
}
