use crate::{Detector, SuggestionDraft};
use servicio_core::worker::{RunMode, Schedule, OverlapPolicy};
use std::path::Path;

pub struct Laravel;

impl Detector for Laravel {
    fn name(&self) -> &str { "laravel" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        if !root.join("artisan").exists() { return vec![]; }
        let composer = std::fs::read_to_string(root.join("composer.json")).unwrap_or_default();
        let mut out = vec![
            draft(root, "Laravel queue worker", "artisan queue:work",
                  vec!["artisan".into(), "queue:work".into()],
                  RunMode::Daemon { concurrency: 2 }),
            draft(root, "Laravel scheduler", "artisan schedule:run",
                  vec!["artisan".into(), "schedule:run".into()],
                  RunMode::Scheduled { schedule: Schedule::Cron("* * * * *".into()), overlap: OverlapPolicy::Skip }),
        ];
        if composer.contains("laravel/horizon") {
            out.push(draft(root, "Laravel Horizon", "artisan horizon",
                           vec!["artisan".into(), "horizon".into()], RunMode::Daemon { concurrency: 1 }));
        }
        if composer.contains("laravel/reverb") {
            out.push(draft(root, "Laravel Reverb", "artisan reverb:start",
                           vec!["artisan".into(), "reverb:start".into()], RunMode::Daemon { concurrency: 1 }));
        }
        out
    }
}

fn draft(root: &Path, label: &str, _shown: &str, args: Vec<String>, run_mode: RunMode) -> SuggestionDraft {
    SuggestionDraft {
        label: label.into(), source: "laravel/artisan".into(),
        name: label.to_lowercase().replace(' ', "-"),
        command: "php".into(), args, working_dir: root.to_path_buf(), run_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_queue_and_scheduler_when_artisan_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "#!/usr/bin/env php").unwrap();
        let s = Laravel.detect(dir.path());
        assert!(s.iter().any(|d| d.args == vec!["artisan", "queue:work"]));
        assert!(s.iter().any(|d| matches!(d.run_mode, RunMode::Scheduled { .. })));
    }
    #[test]
    fn no_artisan_no_suggestions() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Laravel.detect(dir.path()).is_empty());
    }
    #[test]
    fn horizon_detected_from_composer() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::write(dir.path().join("composer.json"), r#"{"require":{"laravel/horizon":"^5"}}"#).unwrap();
        let s = Laravel.detect(dir.path());
        assert!(s.iter().any(|d| d.args == vec!["artisan", "horizon"]));
    }
}
