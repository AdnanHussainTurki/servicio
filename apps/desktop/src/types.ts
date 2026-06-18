export type InstanceState =
  | "stopped" | "starting" | "running" | "stopping" | "crashed" | "backoff" | "failed"
  | "idle" | "completed";

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
  group?: string | null;
  tags?: string[];
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
export type RunModeAny =
  | { type: "daemon"; concurrency: number }
  | { type: "scheduled"; schedule: { cron: string } | { interval_secs: number }; overlap: "skip" | "queue" | "kill_previous" }
  | { type: "batch"; run_count: number; delay_secs: number };

export interface SuggestionDraft {
  label: string; source: string; name: string;
  command: string; args: string[]; working_dir: string; run_mode: RunModeAny;
  group?: string | null; tags?: string[];
}
export interface MetricPointT { ts: number; cpu: number; mem: number }
export interface MetricEventPayload { kind: "metric"; worker: string; instance: number; ts: number; cpu: number; mem: number }

export type WorkerEvent = StateEventPayload | LogEventPayload | MetricEventPayload;
