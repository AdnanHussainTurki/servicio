use crate::{folder_group, Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Procfile;

impl Detector for Procfile {
    fn name(&self) -> &str { "procfile" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let body = match std::fs::read_to_string(root.join("Procfile")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let mut out = vec![];
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let Some((name, cmd)) = line.split_once(':') else { continue };
            let cmd = cmd.trim();
            let mut parts = cmd.split_whitespace();
            let Some(command) = parts.next() else { continue };
            let args: Vec<String> = parts.map(|s| s.to_string()).collect();
            out.push(SuggestionDraft {
                label: format!("Procfile: {}", name.trim()), source: "Procfile".into(),
                name: name.trim().to_string(), command: command.to_string(), args,
                working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
                group: folder_group(root), tags: vec!["procfile".into()],
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_each_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: node server.js\nworker: node worker.js\n").unwrap();
        let s = Procfile.detect(dir.path());
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].name, "worker");
        assert_eq!(s[1].command, "node");
        assert_eq!(s[1].args, vec!["worker.js".to_string()]);
    }
}
