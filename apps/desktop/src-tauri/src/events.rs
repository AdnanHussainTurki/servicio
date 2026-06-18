use serde_json::{json, Value};
use servicio_ipc::Frame;

/// Map a daemon Event frame to the `worker-event` payload the frontend consumes.
pub fn event_payload(frame: &Frame) -> Option<Value> {
    match frame {
        Frame::Event { topic, payload } => {
            let mut obj = payload.clone();
            if let Value::Object(map) = &mut obj {
                map.insert("kind".into(), json!(topic));
            }
            Some(obj)
        }
        _ => None,
    }
}

/// Subscribe to state+log and call `emit` for each mapped payload until the stream closes.
pub async fn run_event_pump<F>(base: std::path::PathBuf, token: String, emit: F)
where
    F: Fn(Value) + Send + 'static,
{
    use servicio_cli_lib::Client;
    let client = match Client::connect(&base, &token).await {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut lines = match client.subscribe(&["state", "log", "metric"], None).await {
        Ok(l) => l,
        Err(_) => return,
    };
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(frame) = Frame::from_line(&line) {
            if let Some(p) = event_payload(&frame) {
                emit(p);
            }
        }
    }
}
