use crate::{folder_group, Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Python;

impl Detector for Python {
    fn name(&self) -> &str { "python" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let has_reqs = root.join("requirements.txt").exists() || root.join("pyproject.toml").exists();
        let has_venv = root.join(".venv/bin/python").exists() || root.join("venv/bin/python").exists();
        // pick an entry script
        let candidates = ["worker.py", "main.py", "app.py", "manage.py"];
        let entry = candidates.iter().find(|c| root.join(c).exists()).map(|s| s.to_string())
            .or_else(|| first_py(root));
        let Some(entry) = entry else {
            // python project but no obvious script
            if has_reqs || has_venv {
                return vec![SuggestionDraft {
                    label: "Python worker".into(), source: "python".into(), name: "python-worker".into(),
                    command: "python".into(), args: vec![], working_dir: root.to_path_buf(),
                    run_mode: RunMode::Daemon { concurrency: 1 },
                    group: folder_group(root), tags: vec!["python".into()],
                }];
            }
            return vec![];
        };
        vec![SuggestionDraft {
            label: format!("Python: {entry}"), source: "python".into(),
            name: entry.trim_end_matches(".py").replace(['/', '\\'], "-"),
            command: "python".into(), args: vec![entry.clone()],
            working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
            group: folder_group(root), tags: vec!["python".into()],
        }]
    }
}

fn first_py(root: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(root).ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".py"))
        .collect();
    names.sort();
    names.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_named_entry_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        std::fs::write(dir.path().join("worker.py"), "print(1)").unwrap();
        let s = Python.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].args, vec!["worker.py".to_string()]);
        assert_eq!(s[0].command, "python");
    }
    #[test]
    fn no_python_no_suggestion() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Python.detect(dir.path()).is_empty());
    }
}
