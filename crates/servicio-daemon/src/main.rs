use clap::Parser;
use servicio_daemon_lib::cli::{Cli, Command};
use servicio_daemon_lib::{add_worker, db::Db};

fn init_daemon_logging(path: &std::path::Path) {
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter, Registry};

    #[derive(Clone)]
    struct FileWriter(Arc<Mutex<std::fs::File>>);
    impl std::io::Write for FileWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }
    impl<'a> MakeWriter<'a> for FileWriter {
        type Writer = FileWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let writer = FileWriter(Arc::new(Mutex::new(file)));
    let fmt_layer = fmt::layer().with_ansi(false).with_writer(writer);
    let _ = Registry::default()
        .with(EnvFilter::new("info"))
        .with(fmt_layer)
        .with(sentry_tracing::layer()) // forwards ERROR events to Sentry (no-op if Sentry uninitialized)
        .try_init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Optional Sentry error reporting. No-op unless SERVICIO_SENTRY_DSN is set.
    let _sentry_guard = std::env::var("SERVICIO_SENTRY_DSN")
        .ok()
        .filter(|d| !d.is_empty())
        .map(|dsn| {
            sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: Some(env!("CARGO_PKG_VERSION").into()),
                    ..Default::default()
                },
            ))
        });

    let cli = Cli::parse();
    match cli.command {
        Command::BuildId => {
            println!("{}", env!("SERVICIO_BUILD"));
        }
        Command::Add {
            name,
            command,
            args,
            working_dir,
            concurrency,
            autostart,
        } => {
            add_worker(
                &cli.db,
                &name,
                &command,
                &args,
                &working_dir,
                concurrency,
                autostart,
            )?;
            println!("added worker '{name}'");
        }
        Command::List => {
            let db = Db::open(&cli.db)?;
            for w in db.list_workers()? {
                println!(
                    "{}  cmd={} {:?}  mode={:?}  autostart={}",
                    w.name, w.command, w.args, w.run_mode, w.autostart
                );
            }
        }
        Command::Serve { base } => {
            use servicio_daemon_lib::lock::InstanceLock;
            use servicio_daemon_lib::paths::Paths;
            use servicio_daemon_lib::serve::serve;
            use servicio_daemon_lib::token::load_or_create;

            let paths = Paths::new(base.unwrap_or_else(Paths::default_base));
            std::fs::create_dir_all(&paths.base)?;
            // daemon self-logging → <base>/daemon.log (best-effort)
            init_daemon_logging(&paths.base.join("daemon.log"));
            let _lock = InstanceLock::acquire(&paths.lock())?;
            let token = load_or_create(&paths.token())?;
            tracing::info!("servicio daemon starting (base={})", paths.base.display());
            let handle = serve(paths, token).await?;
            println!("servicio daemon listening; press Ctrl-C to stop");
            let stop = handle.shutdown_notify();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = stop.notified() => { tracing::info!("shutdown requested via IPC"); }
            }
            tracing::info!("servicio daemon shutting down");
            handle.shutdown().await;
            println!("stopped");
        }
        Command::InstallService { base } => {
            use servicio_daemon_lib::paths::Paths;
            use servicio_daemon_lib::{service, service_spec};
            let base = base.unwrap_or_else(Paths::default_base);
            let dir = service::default_service_dir().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "service install not supported on this OS",
                )
            })?;
            let spec = service_spec(base)?;
            let path = service::install_to(&spec, &dir, true)?;
            println!("installed service: {}", path.display());
        }
        Command::UninstallService => {
            use servicio_daemon_lib::service;
            let dir = service::default_service_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this OS")
            })?;
            service::uninstall_from(&dir, "com.servicio.daemon", true)?;
            println!("uninstalled service");
        }
        Command::StopService => {
            use servicio_daemon_lib::service;
            let dir = service::default_service_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this OS")
            })?;
            service::set_loaded(&dir, "com.servicio.daemon", false)?;
            println!("stopped service");
        }
        Command::StartService => {
            use servicio_daemon_lib::service;
            let dir = service::default_service_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this OS")
            })?;
            service::set_loaded(&dir, "com.servicio.daemon", true)?;
            println!("started service");
        }
        Command::ServiceStatus => {
            use servicio_daemon_lib::service;
            match service::default_service_dir() {
                Some(dir) => {
                    let installed = service::is_installed(&dir, "com.servicio.daemon");
                    println!("{{\"installed\": {installed}}}");
                }
                None => println!("{{\"installed\": false, \"supported\": false}}"),
            }
        }
    }
    Ok(())
}
