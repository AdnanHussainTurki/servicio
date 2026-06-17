import type { WorkerStatus, InstanceState } from "../types";

const DOT: Record<InstanceState, string> = {
  running: "bg-green-500", starting: "bg-amber-400", backoff: "bg-amber-400",
  stopping: "bg-amber-400", stopped: "bg-slate-400", crashed: "bg-red-500", failed: "bg-red-600",
};

function worstState(w: WorkerStatus): InstanceState {
  const order: InstanceState[] = ["failed", "crashed", "backoff", "starting", "stopping", "running", "stopped"];
  for (const s of order) if (w.instances.some((i) => i.state === s)) return s;
  return "stopped";
}

export function WorkerCard({
  w, onOpen, onStart, onStop,
}: { w: WorkerStatus; onOpen: () => void; onStart: () => void; onStop: () => void }) {
  const state = worstState(w);
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  const running = w.instances.filter((i) => i.state === "running").length;
  return (
    <div onClick={onOpen}
      className="cursor-pointer rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 shadow-sm hover:shadow-md transition">
      <div className="flex items-center gap-2">
        <span className={`h-2.5 w-2.5 rounded-full ${DOT[state]}`} />
        <span className="font-semibold flex-1 truncate">{w.name}</span>
      </div>
      <div className="mt-1 text-xs opacity-60">daemon ×{w.run_mode.concurrency}</div>
      <div className="mt-2 text-sm">{state} · {running}/{w.instances.length} up · {restarts} restarts</div>
      <div className="mt-3 flex gap-2" onClick={(e) => e.stopPropagation()}>
        <button className="text-xs rounded bg-green-600 hover:bg-green-700 text-white px-2 py-1 transition" onClick={onStart}>Start</button>
        <button className="text-xs rounded bg-slate-200 hover:bg-slate-300 dark:bg-slate-700 dark:hover:bg-slate-600 px-2 py-1 transition" onClick={onStop}>Stop</button>
      </div>
    </div>
  );
}
