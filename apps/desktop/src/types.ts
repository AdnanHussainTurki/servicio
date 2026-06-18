export type InstanceState =
  | "stopped" | "starting" | "running" | "stopping" | "crashed" | "backoff" | "failed";

export interface InstanceStatus {
  index: number;
  state: InstanceState;
  restart_count: number;
  pid: number | null;
}
export interface RunModeDaemon { type: "daemon"; concurrency: number }
export type RunMode = RunModeDaemon;

export interface WorkerStatus {
  name: string;
  run_mode: RunMode;
  instances: InstanceStatus[];
}
export interface DaemonStatus {
  connected: boolean;
  version: string;
  uptime_secs: number;
  worker_count: number;
  running_count: number;
}
export interface StateEventPayload {
  kind: "state"; worker: string; instance: number; from: InstanceState; to: InstanceState;
}
export interface LogEventPayload {
  kind: "log"; worker: string; instance: number; stream: string; line: string;
}
export type WorkerEvent = StateEventPayload | LogEventPayload;
