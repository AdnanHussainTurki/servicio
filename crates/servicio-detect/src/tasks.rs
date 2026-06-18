use crate::{folder_group, Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Tasks;

impl Detector for Tasks {
    fn name(&self) -> &str {
        "vscode-tasks"
    }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let raw = match std::fs::read_to_string(root.join(".vscode/tasks.json")) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let cleaned = strip_jsonc(&raw);
        let v: serde_json::Value = match serde_json::from_str(&cleaned) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let Some(entries) = v.get("tasks").and_then(|t| t.as_array()) else {
            return vec![];
        };

        let mut out = vec![];
        for entry in entries {
            let Some(label) = entry.get("label").and_then(|l| l.as_str()) else {
                continue;
            };
            let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) else {
                continue;
            };
            let args: Vec<String> = entry
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let (command, final_args) = if cmd.contains(['|', '&', ';', '>', '<', '$']) {
                let mut joined = cmd.to_string();
                if !args.is_empty() {
                    joined.push(' ');
                    joined.push_str(&args.join(" "));
                }
                ("sh".to_string(), vec!["-c".to_string(), joined])
            } else {
                (cmd.to_string(), args)
            };

            out.push(SuggestionDraft {
                label: format!("Task: {label}"),
                source: "vscode/tasks.json".into(),
                name: format!("task-{label}").to_lowercase().replace(' ', "-"),
                command,
                args: final_args,
                working_dir: root.to_path_buf(),
                run_mode: RunMode::Daemon { concurrency: 1 },
                group: folder_group(root),
                tags: vec!["vscode-task".into()],
            });
        }
        out
    }
}

/// Strip JSONC niceties: line/block comments and trailing commas.
fn strip_jsonc(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] as char == '/' {
            // line comment
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] as char == '*' {
            // block comment
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] as char == '*' && bytes[i + 1] as char == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    strip_trailing_commas(&out)
}

/// Remove commas that immediately precede a closing `}` or `]` (ignoring whitespace).
fn strip_trailing_commas(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            // peek ahead past whitespace
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] as char == '}' || bytes[j] as char == ']') {
                i += 1; // drop the comma
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_jsonc_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let vs = dir.path().join(".vscode");
        std::fs::create_dir_all(&vs).unwrap();
        std::fs::write(
            vs.join("tasks.json"),
            r#"{
  // build tasks
  "version": "2.0.0",
  "tasks": [
    { "label": "queue", "type": "shell", "command": "php", "args": ["artisan", "queue:work"], },
  ]
}"#,
        )
        .unwrap();
        let s = Tasks.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].command, "php");
        assert_eq!(
            s[0].args,
            vec!["artisan".to_string(), "queue:work".to_string()]
        );
        assert!(s[0].tags.contains(&"vscode-task".to_string()));
        assert_eq!(s[0].label, "Task: queue");
    }
    #[test]
    fn shell_metachars_wrap_in_sh_c() {
        let dir = tempfile::tempdir().unwrap();
        let vs = dir.path().join(".vscode");
        std::fs::create_dir_all(&vs).unwrap();
        std::fs::write(
            vs.join("tasks.json"),
            r#"{
  "tasks": [
    { "label": "pipe", "command": "cat foo | grep bar" }
  ]
}"#,
        )
        .unwrap();
        let s = Tasks.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].command, "sh");
        assert_eq!(
            s[0].args,
            vec!["-c".to_string(), "cat foo | grep bar".to_string()]
        );
    }
    #[test]
    fn skips_entries_missing_command() {
        let dir = tempfile::tempdir().unwrap();
        let vs = dir.path().join(".vscode");
        std::fs::create_dir_all(&vs).unwrap();
        std::fs::write(
            vs.join("tasks.json"),
            r#"{
  "tasks": [
    { "label": "no-command" },
    { "label": "ok", "command": "echo" }
  ]
}"#,
        )
        .unwrap();
        let s = Tasks.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].command, "echo");
    }
    #[test]
    fn no_tasks_file_no_suggestions() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Tasks.detect(dir.path()).is_empty());
    }
}
