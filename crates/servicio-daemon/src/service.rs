use std::io;
use std::path::{Path, PathBuf};

/// Spec for the service definition: which exe to run + base dir.
pub struct ServiceSpec {
    pub label: String,
    pub exe: std::path::PathBuf,
    pub base: std::path::PathBuf,
}

/// macOS LaunchAgent plist: runs `<exe> serve --base <base>`, at login, kept alive.
pub fn launchd_plist(spec: &ServiceSpec) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>serve</string>
        <string>--base</string>
        <string>{base}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
"#,
        label = spec.label,
        exe = spec.exe.display(),
        base = spec.base.display(),
    )
}

/// systemd user unit: runs `<exe> serve --base <base>`, restart always, at login.
pub fn systemd_unit(spec: &ServiceSpec) -> String {
    format!(
        "[Unit]\nDescription=Servicio supervisor daemon\nAfter=default.target\n\n\
[Service]\nExecStart={exe} serve --base {base}\nRestart=always\nRestartSec=2\n\n\
[Install]\nWantedBy=default.target\n",
        exe = spec.exe.display(),
        base = spec.base.display(),
    )
}

/// Windows Task Scheduler XML: runs `<exe> serve --base <base>` at user logon.
pub fn windows_task_xml(spec: &ServiceSpec) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Servicio supervisor daemon</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>serve --base "{base}"</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        exe = spec.exe.display(),
        base = spec.base.display(),
    )
}

/// Filename for the unit in `dir` (platform-shaped).
fn unit_filename(label: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        format!("{label}.plist")
    }
    #[cfg(target_os = "windows")]
    {
        format!("{label}.task.xml")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = label;
        "servicio.service".to_string()
    }
}

fn unit_body(spec: &ServiceSpec) -> String {
    #[cfg(target_os = "macos")]
    {
        launchd_plist(spec)
    }
    #[cfg(target_os = "windows")]
    {
        windows_task_xml(spec)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        systemd_unit(spec)
    }
}

/// Write the unit file into `dir`. If `load`, invoke the platform loader.
pub fn install_to(spec: &ServiceSpec, dir: &Path, load: bool) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(unit_filename(&spec.label));
    std::fs::write(&path, unit_body(spec))?;
    if load {
        run_loader(&path, &spec.label, true);
    }
    Ok(path)
}

pub fn uninstall_from(dir: &Path, label: &str, load: bool) -> io::Result<()> {
    let path = dir.join(unit_filename(label));
    if load {
        run_loader(&path, label, false);
    }
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn is_installed(dir: &Path, label: &str) -> bool {
    dir.join(unit_filename(label)).exists()
}

/// Best-effort invoke launchctl/systemctl. Errors are ignored (status is informational).
fn run_loader(path: &Path, label: &str, enable: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = label;
        let arg = if enable { "load" } else { "unload" };
        let _ = std::process::Command::new("launchctl")
            .arg(arg)
            .arg("-w")
            .arg(path)
            .status();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = path;
        let action = if enable { "enable" } else { "disable" };
        let _ = std::process::Command::new("systemctl")
            .arg("--user")
            .arg(action)
            .arg("--now")
            .arg("servicio.service")
            .status();
        let _ = label;
    }
    #[cfg(target_os = "windows")]
    {
        if enable {
            // Register (or replace) the logon task from the written XML descriptor.
            let _ = std::process::Command::new("schtasks")
                .args(["/create", "/tn", label, "/xml"])
                .arg(path)
                .arg("/f")
                .status();
        } else {
            let _ = std::process::Command::new("schtasks")
                .args(["/delete", "/tn", label, "/f"])
                .status();
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (path, label, enable);
    }
}

/// Default platform dir for the unit file.
pub fn default_service_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs_home().map(|h| h.join("Library/LaunchAgents"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs_config().map(|c| c.join("systemd/user"))
    }
    #[cfg(target_os = "windows")]
    {
        // Stash the task XML under %LOCALAPPDATA%\Servicio (fallback to temp).
        Some(
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("Servicio"),
        )
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
#[cfg(target_os = "linux")]
fn dirs_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec() -> ServiceSpec {
        ServiceSpec {
            label: "com.servicio.daemon".into(),
            exe: PathBuf::from("/usr/local/bin/servicio-daemon"),
            base: PathBuf::from("/tmp/servicio"),
        }
    }

    #[test]
    fn launchd_plist_has_run_at_load_and_program_args() {
        let p = launchd_plist(&spec());
        assert!(p.contains("<key>RunAtLoad</key>"));
        assert!(p.contains("<true/>"));
        assert!(p.contains("com.servicio.daemon"));
        assert!(p.contains("/usr/local/bin/servicio-daemon"));
        assert!(p.contains("serve"));
        assert!(p.contains("--base"));
        assert!(p.contains("/tmp/servicio"));
    }

    #[test]
    fn systemd_unit_has_execstart_and_wantedby() {
        let u = systemd_unit(&spec());
        assert!(u.contains("ExecStart=/usr/local/bin/servicio-daemon serve --base /tmp/servicio"));
        assert!(u.contains("Restart=always"));
        assert!(u.contains("WantedBy=default.target"));
    }

    #[test]
    fn install_writes_unit_file_to_dir_without_loading() {
        let dir = tempfile::tempdir().unwrap();
        let path = install_to(&spec(), dir.path(), false).unwrap();
        assert!(path.exists());
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("servicio-daemon"));
        // status sees it installed
        assert!(is_installed(dir.path(), &spec().label));
        // uninstall removes it
        uninstall_from(dir.path(), &spec().label, false).unwrap();
        assert!(!path.exists());
        assert!(!is_installed(dir.path(), &spec().label));
    }
}
