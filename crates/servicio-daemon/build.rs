use std::process::Command;

fn main() {
    // A per-build identifier: short git hash (+ "-dirty"), else build epoch seconds.
    let build = git_build_id().unwrap_or_else(|| {
        format!(
            "t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        )
    });
    println!("cargo:rustc-env=SERVICIO_BUILD={build}");

    // Re-stamp whenever HEAD moves. build.rs runs with CWD = the crate dir, but `.git`
    // lives at the workspace root — so we resolve the ABSOLUTE git dir and watch
    // `logs/HEAD`, which is appended on every commit / checkout / merge / reset. (Watching
    // a relative `.git/HEAD` here points at a path that doesn't exist next to the crate,
    // which cargo treats as "never changes" → the build script caches and the id freezes.)
    if let Some(gitdir) = git_dir() {
        println!("cargo:rerun-if-changed={gitdir}/HEAD");
        println!("cargo:rerun-if-changed={gitdir}/logs/HEAD");
        println!("cargo:rerun-if-changed={gitdir}/index");
    }
}

fn git_build_id() -> Option<String> {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())?;
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    Some(if dirty { format!("{hash}-dirty") } else { hash })
}

fn git_dir() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
