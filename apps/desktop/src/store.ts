import { create } from "zustand";
import type { WorkerStatus, WorkerEvent, DaemonStatus } from "./types";

const LOG_CAP = 1000;

interface State {
  workers: Record<string, WorkerStatus>;
  logs: Record<string, string[]>;
  daemon: DaemonStatus | null;
  setWorkers: (list: WorkerStatus[]) => void;
  setDaemon: (d: DaemonStatus) => void;
  applyEvent: (e: WorkerEvent) => void;
  reset: () => void;
}

export const useStore = create<State>((set) => ({
  workers: {},
  logs: {},
  daemon: null,
  setWorkers: (list) =>
    set(() => ({ workers: Object.fromEntries(list.map((w) => [w.name, w])) })),
  setDaemon: (daemon) => set(() => ({ daemon })),
  applyEvent: (e) =>
    set((s) => {
      if (e.kind === "state") {
        const w = s.workers[e.worker];
        if (!w) return {};
        const instances = w.instances.map((i) =>
          i.index === e.instance ? { ...i, state: e.to } : i
        );
        return { workers: { ...s.workers, [e.worker]: { ...w, instances } } };
      } else {
        const prev = s.logs[e.worker] ?? [];
        const next = [...prev, `[${e.stream}] ${e.line}`];
        if (next.length > LOG_CAP) next.splice(0, next.length - LOG_CAP);
        return { logs: { ...s.logs, [e.worker]: next } };
      }
    }),
  reset: () => set(() => ({ workers: {}, logs: {}, daemon: null })),
}));
