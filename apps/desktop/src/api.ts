import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WorkerStatus, DaemonStatus, WorkerEvent, RunMode, SuggestionDraft, MetricPointT } from "./types";
import { useStore } from "./store";
import { notifyStateEvent } from "./notify";

export interface AddWorkerSpec {
  name: string;
  command: string;
  args: string[];
  working_dir: string;
  env: Record<string, string>;
  run_mode: RunMode;
  restart: { kind: string; max_retries: number; base_secs: number; max_secs: number; reset_window_secs: number };
  autostart: boolean;
  enabled: boolean;
}

export const api = {
  daemonStatus: () => invoke<DaemonStatus>("daemon_status"),
  listWorkers: () => invoke<WorkerStatus[]>("list_workers"),
  startWorker: (name: string) => invoke<void>("start_worker", { name }),
  stopWorker: (name: string) => invoke<void>("stop_worker", { name }),
  restartWorker: (name: string) => invoke<void>("restart_worker", { name }),
  addWorker: (spec: AddWorkerSpec) => invoke<void>("add_worker", { spec }),
  detectWorkers: (path: string) => invoke<SuggestionDraft[]>("detect_workers", { path }),
  metrics: (worker: string, sinceSecs: number) => invoke<{ instance: number; points: MetricPointT[] }[]>("metrics", { worker, sinceSecs }),
  serviceStatus: () => invoke<{ installed: boolean; supported?: boolean }>("service_status"),
  installService: () => invoke<void>("install_service"),
  uninstallService: () => invoke<void>("uninstall_service"),
};

/** Wire daemon events into the store. Call once at app start. */
export async function subscribeEvents() {
  try {
    await listen<WorkerEvent>("worker-event", (ev) => {
      const p = ev.payload;
      useStore.getState().applyEvent(p);
      if (p.kind === "state") { void notifyStateEvent(p); }
    });
  } catch (err) {
    console.warn("subscribeEvents failed:", err);
    return;
  }
}

/**
 * Await a command promise; on rejection record the error in the store
 * (for the error toast) and resolve to undefined instead of throwing.
 */
export async function withError<T>(p: Promise<T>): Promise<T | undefined> {
  try {
    return await p;
  } catch (err) {
    useStore.getState().setError(String(err));
    return undefined;
  }
}
