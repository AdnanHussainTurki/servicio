use clap::Parser;
use servicio_daemon_lib::cli::{Cli, Command};
use servicio_daemon_lib::{add_worker, db::Db};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Add { name, command, args, working_dir, concurrency, autostart } => {
            add_worker(&cli.db, &name, &command, &args, &working_dir, concurrency, autostart)?;
            println!("added worker '{name}'");
        }
        Command::List => {
            let db = Db::open(&cli.db)?;
            for w in db.list_workers()? {
                println!("{}  cmd={} {:?}  mode={:?}  autostart={}", w.name, w.command, w.args, w.run_mode, w.autostart);
            }
        }
        Command::Serve { base } => {
            use servicio_daemon_lib::lock::InstanceLock;
            use servicio_daemon_lib::paths::Paths;
            use servicio_daemon_lib::serve::serve;
            use servicio_daemon_lib::token::load_or_create;

            let paths = Paths::new(base.unwrap_or_else(Paths::default_base));
            std::fs::create_dir_all(&paths.base)?;
            let _lock = InstanceLock::acquire(&paths.lock())?;
            let token = load_or_create(&paths.token())?;
            let handle = serve(paths, token).await?;
            println!("servicio daemon listening; press Ctrl-C to stop");
            tokio::signal::ctrl_c().await?;
            handle.shutdown().await;
            println!("stopped");
        }
        Command::InstallService { base } => {
            use servicio_daemon_lib::{service, service_spec};
            use servicio_daemon_lib::paths::Paths;
            let base = base.unwrap_or_else(Paths::default_base);
            let dir = service::default_service_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Unsupported, "service install not supported on this OS"))?;
            let spec = service_spec(base)?;
            let path = service::install_to(&spec, &dir, true)?;
            println!("installed service: {}", path.display());
        }
        Command::UninstallService => {
            use servicio_daemon_lib::service;
            let dir = service::default_service_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this OS"))?;
            service::uninstall_from(&dir, "com.servicio.daemon", true)?;
            println!("uninstalled service");
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
