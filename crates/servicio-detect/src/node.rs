use crate::{Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Node;

impl Detector for Node {
    fn name(&self) -> &str { "node" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let pkg = match std::fs::read_to_string(root.join("package.json")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let v: serde_json::Value = match serde_json::from_str(&pkg) { Ok(v) => v, Err(_) => return vec![] };
        let mut out = vec![];
        if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
            for (name, _) in scripts {
                let n = name.to_lowercase();
                if n.contains("worker") || n.contains("queue") {
                    out.push(SuggestionDraft {
                        label: format!("npm run {name}"), source: "package.json".into(),
                        name: format!("node-{name}"), command: "npm".into(),
                        args: vec!["run".into(), name.clone()],
                        working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_worker_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"),
            r#"{"scripts":{"worker":"node worker.js","build":"tsc"}}"#).unwrap();
        let s = Node.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].args, vec!["run".to_string(), "worker".to_string()]);
    }
}
