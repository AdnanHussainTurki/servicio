use crate::serve::Daemon;
use servicio_core::event::SupervisorEvent;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};

/// Sample every live instance's cpu/mem every 2s: store to DB + emit Metric events.
/// Prunes rows older than `retain_secs` roughly hourly. Runs until the process exits.
pub async fn run_sampler_for(daemon: Arc<Daemon>, retain_secs: u64) {
    let mut sys = System::new();
    let mut ticks: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (snapshot, sender) = {
            let mgr = daemon.manager.lock().await;
            (mgr.status(), mgr.events_sender())
        };
        for w in snapshot {
            for inst in w.instances {
                if let Some(pid) = inst.pid {
                    if let Some(p) = sys.process(Pid::from_u32(pid)) {
                        let cpu = p.cpu_usage();
                        let mem = p.memory();
                        {
                            let db = daemon.db.lock().await;
                            let _ = db.insert_metric(&w.name, inst.index, now, cpu, mem);
                        }
                        let _ = sender.send(SupervisorEvent::Metric {
                            worker: w.name.clone(),
                            instance: inst.index,
                            ts: now,
                            cpu,
                            mem,
                        });
                    }
                }
            }
        }
        ticks += 1;
        if ticks.is_multiple_of(30) {
            let cutoff = now.saturating_sub(retain_secs);
            let db = daemon.db.lock().await;
            let _ = db.prune_metrics(cutoff);
        }
    }
}
