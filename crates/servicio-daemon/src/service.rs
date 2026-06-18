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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec() -> ServiceSpec {
        ServiceSpec { label: "com.servicio.daemon".into(), exe: PathBuf::from("/usr/local/bin/servicio-daemon"), base: PathBuf::from("/tmp/servicio") }
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
}
