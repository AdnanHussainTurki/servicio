import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WorkerStatus, DaemonStatus, WorkerEvent, RunMode, SuggestionDraft, MetricPointT } from "./types";
import { useStore } from "./store";
import { notifyStateEvent } from "./notify";
import type { EditSpec } from "./components/CreateFlow";

export interface AddWorkerSpec {
  name: string;
  display_name?: string | null;
  command: string;
  args: string[];
  working_dir: string;
  env: Record<string, string>;
  run_mode: RunMode;
  restart: { kind: string; max_retries: number; base_secs: number; max_secs: number; reset_window_secs: number };
  autostart: boolean;
  enabled: boolean;
  group?: string | null;
  tags?: string[];
}

export const api = {
  daemonStatus: () => invoke<DaemonStatus>("daemon_status"),
  listWorkers: () => invoke<WorkerStatus[]>("list_workers"),
  startWorker: (name: string) => invoke<void>("start_worker", { name }),
  stopWorker: (name: string) => invoke<void>("stop_worker", { name }),
  restartWorker: (name: string) => invoke<void>("restart_worker", { name }),
  startGroup: (group: string) => invoke<{ started: number }>("start_group", { group }),
  stopGroup: (group: string) => invoke<{ stopped: number }>("stop_group", { group }),
  addWorker: (spec: AddWorkerSpec) => invoke<void>("add_worker", { spec }),
  removeWorker: (name: string) => invoke<void>("remove_worker", { name }),
  exportWorkersTo: (path: string) => invoke<number>("export_workers_to", { path }),
  importWorkersFrom: (path: string) => invoke<number>("import_workers_from", { path }),
  getWorker: (name: string) => invoke<EditSpec>("get_worker", { name }),
  detectWorkers: (path: string) => invoke<SuggestionDraft[]>("detect_workers", { path }),
  metrics: (worker: string, sinceSecs: number) => invoke<{ instance: number; points: MetricPointT[] }[]>("metrics", { worker, sinceSecs }),
  serviceStatus: () => invoke<{ installed: boolean; supported?: boolean }>("service_status"),
  appVersion: () => invoke<string>("app_version"),
  daemonLog: (lines: number) => invoke<{ log: string }>("daemon_log", { lines }),
  pickFolder: async (): Promise<string | null> => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, multiple: false });
      return typeof r === "string" ? r : null;
    } catch { return null; }
  },
  saveDialog: async (defaultName: string): Promise<string | null> => {
    try { const { save } = await import("@tauri-apps/plugin-dialog"); const p = await save({ defaultPath: defaultName, filters: [{ name: "JSON", extensions: ["json"] }] }); return p ?? null; } catch { return null; }
  },
  openFileDialog: async (): Promise<string | null> => {
    try { const { open } = await import("@tauri-apps/plugin-dialog"); const r = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] }); return typeof r === "string" ? r : null; } catch { return null; }
  },
  installService: () => invoke<void>("install_service"),
  uninstallService: () => invoke<void>("uninstall_service"),
  checkUpdate: async (): Promise<string | null> => {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const u = await check();
      return u ? `Update available: ${u.version}` : "Up to date";
    } catch (e) { return `Updater unavailable: ${String(e)}`; }
  },
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
