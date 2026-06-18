use std::process::Command;
fn main() {
    // A per-build identifier: short git hash (+ "-dirty"), else build epoch seconds.
    let git = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    let build = match git {
        Some(h) if !h.is_empty() => {
            if dirty {
                format!("{h}-dirty")
            } else {
                h
            }
        }
        _ => format!(
            "t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ),
    };
    println!("cargo:rustc-env=SERVICIO_BUILD={build}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
