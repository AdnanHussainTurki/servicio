import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WorkerStatus, DaemonStatus, WorkerEvent, RunMode } from "./types";
import { useStore } from "./store";

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
};

/** Wire daemon events into the store. Call once at app start. */
export async function subscribeEvents() {
  await listen<WorkerEvent>("worker-event", (ev) => {
    useStore.getState().applyEvent(ev.payload);
  });
}
