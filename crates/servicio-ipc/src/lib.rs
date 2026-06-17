//! servicio-ipc: the wire protocol shared by the daemon and clients.
//! Pure types + line framing. No tokio, no IO.

pub mod types;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One protocol message. Encoded as a single JSON object on its own line (JSONL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Frame {
    Request { id: u64, method: String, params: Value },
    Response { id: u64, result: Option<Value>, error: Option<ApiError> },
    Event { topic: String, payload: Value },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl Frame {
    /// Serialize to a single line WITHOUT the trailing newline.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("frame serializes")
    }

    /// Parse one line (newline already stripped) into a Frame.
    pub fn from_line(line: &str) -> Result<Frame, serde_json::Error> {
        serde_json::from_str(line)
    }

    pub fn ok(id: u64, result: Value) -> Frame {
        Frame::Response { id, result: Some(result), error: None }
    }

    pub fn err(id: u64, code: &str, message: &str) -> Frame {
        Frame::Response {
            id,
            result: None,
            error: Some(ApiError { code: code.into(), message: message.into() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_roundtrips_and_has_no_newline() {
        let f = Frame::Request { id: 7, method: "ping".into(), params: json!({}) };
        let line = f.to_line();
        assert!(!line.contains('\n'));
        assert_eq!(Frame::from_line(&line).unwrap(), f);
    }

    #[test]
    fn ok_and_err_helpers_build_responses() {
        assert_eq!(
            Frame::ok(1, json!({"pong": true})),
            Frame::Response { id: 1, result: Some(json!({"pong": true})), error: None }
        );
        match Frame::err(2, "unauthorized", "bad token") {
            Frame::Response { id: 2, result: None, error: Some(e) } => {
                assert_eq!(e.code, "unauthorized");
            }
            _ => panic!("expected error response"),
        }
    }

    #[test]
    fn event_frame_roundtrips() {
        let f = Frame::Event { topic: "state".into(), payload: json!({"worker": "q"}) };
        assert_eq!(Frame::from_line(&f.to_line()).unwrap(), f);
    }

    #[test]
    fn malformed_line_is_an_error_not_a_panic() {
        assert!(Frame::from_line("{not json").is_err());
    }
}
