use crate::{folder_group, Detector, SuggestionDraft};
use servicio_core::worker::{RunMode, Schedule, OverlapPolicy};
use std::path::Path;

pub struct Crontab;

impl Detector for Crontab {
    fn name(&self) -> &str { "crontab" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let body = match std::fs::read_to_string(root.join("crontab")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let mut out = vec![];
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 { continue; }
            let expr = fields[..5].join(" ");
            let command = fields[5].to_string();
            let args: Vec<String> = fields[6..].iter().map(|s| s.to_string()).collect();
            out.push(SuggestionDraft {
                label: format!("crontab: {command}"), source: "crontab".into(),
                name: format!("cron-{command}").replace(['/', '.'], "-"),
                command, args, working_dir: root.to_path_buf(),
                run_mode: RunMode::Scheduled { schedule: Schedule::Cron(expr), overlap: OverlapPolicy::Skip },
                group: folder_group(root), tags: vec!["crontab".into()],
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cron_lines_to_scheduled() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("crontab"), "0 3 * * * python cleanup.py\n# comment\n").unwrap();
        let s = Crontab.detect(dir.path());
        assert_eq!(s.len(), 1);
        match &s[0].run_mode {
            RunMode::Scheduled { schedule: Schedule::Cron(e), .. } => assert_eq!(e, "0 3 * * *"),
            other => panic!("expected scheduled cron, got {other:?}"),
        }
        assert_eq!(s[0].command, "python");
        assert_eq!(s[0].args, vec!["cleanup.py".to_string()]);
    }
}
