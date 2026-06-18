import { create } from "zustand";
import type { WorkerStatus, WorkerEvent, DaemonStatus, MetricPointT } from "./types";

const LOG_CAP = 1000;
const METRIC_CAP = 200;

interface State {
  workers: Record<string, WorkerStatus>;
  logs: Record<string, string[]>;
  metrics: Record<string, MetricPointT[]>;
  latestMetric: Record<string, Record<number, { cpu: number; mem: number }>>;
  daemon: DaemonStatus | null;
  lastError: string | null;
  daemonWarning: string | null;
  setWorkers: (list: WorkerStatus[]) => void;
  setDaemon: (d: DaemonStatus) => void;
  setError: (msg: string | null) => void;
  setDaemonWarning: (msg: string | null) => void;
  applyEvent: (e: WorkerEvent) => void;
  reset: () => void;
}

export const useStore = create<State>((set) => ({
  workers: {},
  logs: {},
  metrics: {},
  latestMetric: {},
  daemon: null,
  lastError: null,
  daemonWarning: null,
  setWorkers: (list) =>
    set(() => ({ workers: Object.fromEntries(list.map((w) => [w.name, w])) })),
  setDaemon: (daemon) => set(() => ({ daemon })),
  setError: (lastError) => set(() => ({ lastError })),
  setDaemonWarning: (daemonWarning) => set(() => ({ daemonWarning })),
  applyEvent: (e) =>
    set((s) => {
      if (e.kind === "state") {
        const w = s.workers[e.worker];
        if (!w) return {};
        const instances = w.instances.map((i) =>
          i.index === e.instance ? { ...i, state: e.to } : i
        );
        return { workers: { ...s.workers, [e.worker]: { ...w, instances } } };
      } else if (e.kind === "log") {
        const prev = s.logs[e.worker] ?? [];
        const next = [...prev, `[${e.stream}] ${e.line}`];
        if (next.length > LOG_CAP) next.splice(0, next.length - LOG_CAP);
        return { logs: { ...s.logs, [e.worker]: next } };
      } else if (e.kind === "metric") {
        const prev = s.metrics[e.worker] ?? [];
        const next = [...prev, { ts: e.ts, cpu: e.cpu, mem: e.mem }];
        if (next.length > METRIC_CAP) next.splice(0, next.length - METRIC_CAP);
        const prevLatest = s.latestMetric[e.worker] ?? {};
        return {
          metrics: { ...s.metrics, [e.worker]: next },
          latestMetric: {
            ...s.latestMetric,
            [e.worker]: { ...prevLatest, [e.instance]: { cpu: e.cpu, mem: e.mem } },
          },
        };
      }
      return {};
    }),
  reset: () => set(() => ({ workers: {}, logs: {}, metrics: {}, latestMetric: {}, daemon: null, lastError: null, daemonWarning: null })),
}));
