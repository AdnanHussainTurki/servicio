use crate::{folder_group, Detector, SuggestionDraft};
use servicio_core::worker::{OverlapPolicy, RunMode, Schedule};
use std::path::Path;

pub struct Laravel;

impl Detector for Laravel {
    fn name(&self) -> &str { "laravel" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        if !root.join("artisan").exists() { return vec![]; }
        let composer = std::fs::read_to_string(root.join("composer.json")).unwrap_or_default();
        let driver = queue_driver(root);

        let mut out = vec![
            draft(root, "Laravel scheduler", vec!["artisan".into(), "schedule:run".into()],
                  RunMode::Scheduled { schedule: Schedule::Cron("* * * * *".into()), overlap: OverlapPolicy::Skip },
                  vec!["laravel".into(), "scheduler".into()]),
        ];

        // sync runs inline; no dedicated queue worker.
        if driver != "sync" {
            let concurrency = if driver == "redis" || driver == "sqs" { 4 } else { 2 };
            out.insert(0, draft(
                root,
                &format!("Laravel queue ({driver})"),
                vec!["artisan".into(), "queue:work".into(), driver.clone(), "--tries=3".into()],
                RunMode::Daemon { concurrency },
                vec!["laravel".into(), driver.clone()],
            ));
        }

        if composer.contains("laravel/horizon") {
            out.push(draft(root, "Laravel Horizon", vec!["artisan".into(), "horizon".into()],
                           RunMode::Daemon { concurrency: 1 }, vec!["laravel".into(), "horizon".into()]));
        }
        if composer.contains("laravel/reverb") {
            out.push(draft(root, "Laravel Reverb", vec!["artisan".into(), "reverb:start".into()],
                           RunMode::Daemon { concurrency: 1 }, vec!["laravel".into(), "reverb".into()]));
        }
        out
    }
}

/// Resolve the configured queue driver, defaulting to `sync`.
fn queue_driver(root: &Path) -> String {
    if let Ok(env) = std::fs::read_to_string(root.join(".env")) {
        if let Some(v) = env_value(&env, "QUEUE_CONNECTION") {
            return v;
        }
    }
    if let Ok(cfg) = std::fs::read_to_string(root.join("config/queue.php")) {
        if let Some(v) = config_default(&cfg, "QUEUE_CONNECTION") {
            return v;
        }
    }
    "sync".into()
}

/// Find `KEY=value` in a dotenv file, trimming quotes and ignoring comments.
fn env_value(env: &str, key: &str) -> Option<String> {
    for line in env.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() != key { continue; }
        let v = v.trim();
        // strip inline comment
        let v = v.split_once(" #").map(|(a, _)| a.trim()).unwrap_or(v);
        let v = v.trim_matches(|c| c == '"' || c == '\'').trim();
        if v.is_empty() { return None; }
        return Some(v.to_string());
    }
    None
}

/// Capture the default literal in `env('KEY', '<default>')` from a PHP config file.
fn config_default(cfg: &str, key: &str) -> Option<String> {
    let needle = format!("env('{key}'");
    let idx = cfg.find(&needle)?;
    let rest = &cfg[idx + needle.len()..];
    // find first comma after the key
    let comma = rest.find(',')?;
    let after = &rest[comma + 1..];
    // grab the first quoted literal
    let start = after.find(|c| c == '\'' || c == '"')?;
    let quote = after.as_bytes()[start] as char;
    let after = &after[start + 1..];
    let end = after.find(quote)?;
    let val = after[..end].trim();
    if val.is_empty() { None } else { Some(val.to_string()) }
}

fn draft(root: &Path, label: &str, args: Vec<String>, run_mode: RunMode, tags: Vec<String>) -> SuggestionDraft {
    SuggestionDraft {
        label: label.into(),
        source: "laravel/artisan".into(),
        name: label.to_lowercase().replace([' ', '(', ')'], "-").replace("--", "-").trim_matches('-').to_string(),
        command: "php".into(),
        args,
        working_dir: root.to_path_buf(),
        run_mode,
        group: folder_group(root),
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_scheduler_when_artisan_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "#!/usr/bin/env php").unwrap();
        let s = Laravel.detect(dir.path());
        assert!(s.iter().any(|d| matches!(d.run_mode, RunMode::Scheduled { .. })));
        assert!(s.iter().all(|d| d.group.is_some()));
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
        let h = s.iter().find(|d| d.args == vec!["artisan", "horizon"]).unwrap();
        assert!(h.tags.contains(&"horizon".to_string()));
    }
    #[test]
    fn redis_driver_from_env() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::write(dir.path().join(".env"), "APP_ENV=local\nQUEUE_CONNECTION=redis\n").unwrap();
        let s = Laravel.detect(dir.path());
        let q = s.iter().find(|d| d.label.contains("queue")).unwrap();
        assert!(q.args.iter().any(|a| a == "redis"));
        assert!(q.tags.contains(&"redis".to_string()));
        assert_eq!(q.run_mode, RunMode::Daemon { concurrency: 4 });
    }
    #[test]
    fn sync_driver_skips_queue_worker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::write(dir.path().join(".env"), "QUEUE_CONNECTION=sync\n").unwrap();
        let s = Laravel.detect(dir.path());
        assert!(!s.iter().any(|d| d.label.contains("queue")));
        assert!(s.iter().any(|d| d.label.contains("scheduler"))); // scheduler still suggested
    }
    #[test]
    fn no_env_defaults_to_sync_no_queue() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        let s = Laravel.detect(dir.path());
        assert!(!s.iter().any(|d| d.label.contains("queue")));
    }
    #[test]
    fn database_driver_concurrency_two() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::write(dir.path().join(".env"), "QUEUE_CONNECTION=database\n").unwrap();
        let s = Laravel.detect(dir.path());
        let q = s.iter().find(|d| d.label.contains("queue")).unwrap();
        assert_eq!(q.run_mode, RunMode::Daemon { concurrency: 2 });
    }
    #[test]
    fn driver_from_config_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/queue.php"),
            "<?php return ['default' => env('QUEUE_CONNECTION', 'redis')];").unwrap();
        let s = Laravel.detect(dir.path());
        let q = s.iter().find(|d| d.label.contains("queue")).unwrap();
        assert!(q.args.iter().any(|a| a == "redis"));
    }
}
