//! servicio-detect: scan a project folder, suggest workers. Pure, no IO beyond reading `root`.

use serde::{Deserialize, Serialize};
use servicio_core::worker::RunMode;
use std::path::{Path, PathBuf};

mod laravel;
mod python;
mod node;
mod procfile;
mod crontab;
mod tasks;

/// Folder name of `root`, used as the default suggestion group.
pub fn folder_group(root: &std::path::Path) -> Option<String> {
    root.file_name().and_then(|n| n.to_str()).map(|s| s.to_string())
}

/// A proposed worker the user confirms/edits before it's created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestionDraft {
    pub label: String,
    pub source: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub run_mode: RunMode,
    pub group: Option<String>,
    pub tags: Vec<String>,
}

pub trait Detector {
    fn name(&self) -> &str;
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft>;
}

/// Always-present fallback so the wizard can start from scratch.
pub struct Generic;
impl Detector for Generic {
    fn name(&self) -> &str { "generic" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        vec![SuggestionDraft {
            label: "Custom worker".into(),
            source: "generic".into(),
            name: String::new(),
            command: String::new(),
            args: vec![],
            working_dir: root.to_path_buf(),
            run_mode: RunMode::Daemon { concurrency: 1 },
            group: folder_group(root),
            tags: vec![],
        }]
    }
}

/// Run every detector against `root`, dedup by (command, args, working_dir).
pub fn detect_all(root: &Path) -> Vec<SuggestionDraft> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(laravel::Laravel),
        Box::new(python::Python),
        Box::new(node::Node),
        Box::new(procfile::Procfile),
        Box::new(crontab::Crontab),
        Box::new(tasks::Tasks),
        Box::new(Generic),
    ];
    let mut out: Vec<SuggestionDraft> = Vec::new();
    for d in &detectors {
        for s in d.detect(root) {
            let dup = out.iter().any(|e| e.command == s.command && e.args == s.args && e.working_dir == s.working_dir);
            if !dup { out.push(s); }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_always_suggests_one_draft() {
        let dir = tempfile::tempdir().unwrap();
        let s = Generic.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].source, "generic");
    }
    #[test]
    fn detect_all_includes_generic_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let all = detect_all(dir.path());
        assert!(all.iter().any(|s| s.source == "generic"));
    }
    #[test]
    fn detect_all_dedups_identical_commands() {
        let dir = tempfile::tempdir().unwrap();
        // Procfile + Python both could suggest the same script; ensure no exact dup remains.
        std::fs::write(dir.path().join("Procfile"), "worker: python worker.py\n").unwrap();
        std::fs::write(dir.path().join("worker.py"), "print(1)").unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "x").unwrap();
        let all = detect_all(dir.path());
        // python suggests `python ["worker.py"]`; procfile suggests `python ["worker.py"]` — must dedup to one.
        let count = all.iter().filter(|s| s.command == "python" && s.args == vec!["worker.py".to_string()]).count();
        assert_eq!(count, 1);
    }
}
