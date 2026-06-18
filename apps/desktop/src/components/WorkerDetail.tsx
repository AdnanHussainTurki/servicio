import { useStore } from "../store";
import { api, withError } from "../api";
import { LogView } from "./LogView";
import { useState } from "react";

export function WorkerDetail({ name, onBack }: { name: string; onBack: () => void }) {
  const w = useStore((s) => s.workers[name]);
  const [tab, setTab] = useState<"logs" | "config">("logs");
  if (!w) return <div className="p-6">Worker not found. <button onClick={onBack} className="underline">Back</button></div>;
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  return (
    <div className="p-6">
      <button onClick={onBack} className="text-sm underline mb-3">← Back</button>
      <div className="flex items-center gap-3 mb-4">
        <h2 className="text-xl font-semibold">{name}</h2>
        <span className="text-xs opacity-60">daemon ×{w.run_mode.concurrency} · {restarts} restarts</span>
        <span className="flex-1" />
        <button className="rounded bg-green-600 hover:bg-green-700 text-white text-sm px-3 py-1.5 transition" onClick={() => withError(api.startWorker(name))}>Start</button>
        <button className="rounded bg-slate-200 hover:bg-slate-300 dark:bg-slate-700 dark:hover:bg-slate-600 text-sm px-3 py-1.5 transition" onClick={() => withError(api.stopWorker(name))}>Stop</button>
        <button className="rounded bg-slate-200 hover:bg-slate-300 dark:bg-slate-700 dark:hover:bg-slate-600 text-sm px-3 py-1.5 transition" onClick={() => withError(api.restartWorker(name))}>Restart</button>
      </div>
      <div className="flex gap-4 border-b border-slate-200 dark:border-slate-800 mb-3 text-sm">
        <button className={tab === "logs" ? "border-b-2 border-blue-600 pb-1" : "pb-1 opacity-60"} onClick={() => setTab("logs")}>Logs</button>
        <button className={tab === "config" ? "border-b-2 border-blue-600 pb-1" : "pb-1 opacity-60"} onClick={() => setTab("config")}>Config</button>
      </div>
      {tab === "logs" ? <LogView worker={name} /> : (
        <pre className="text-xs bg-slate-100 dark:bg-slate-900 rounded p-3 overflow-auto">
          {JSON.stringify(w, null, 2)}
        </pre>
      )}
    </div>
  );
}
