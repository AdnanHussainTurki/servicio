import { useStore } from "../store";
import { api, withError } from "../api";
import { LogView } from "./LogView";
import { MetricsTab } from "./MetricsTab";
import { StatusDot } from "./StatusDot";
import { worstState, styleFor } from "./status";
import { useState } from "react";

export function WorkerDetail({ name, onBack, onEdit, onDelete }: { name: string; onBack: () => void; onEdit?: () => void; onDelete?: () => void }) {
  const w = useStore((s) => s.workers[name]);
  const [tab, setTab] = useState<"logs" | "metrics" | "config">("logs");

  if (!w)
    return (
      <div className="p-6 text-sm text-stone-500">
        Worker not found.{" "}
        <button onClick={onBack} className="text-signal-600 underline dark:text-signal-400">
          Back
        </button>
      </div>
    );

  const state = worstState(w);
  const s = styleFor(state);
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  const running = w.instances.filter((i) => i.state === "running").length;

  const TabBtn = ({ id, label }: { id: "logs" | "metrics" | "config"; label: string }) => (
    <button
      onClick={() => setTab(id)}
      className={`-mb-px border-b-2 px-1 pb-2.5 text-sm font-medium transition ${
        tab === id
          ? "border-signal-500 text-stone-900 dark:text-stone-50"
          : "border-transparent text-stone-400 hover:text-stone-600 dark:hover:text-stone-300"
      }`}
    >
      {label}
    </button>
  );

  return (
    <div className="mx-auto max-w-5xl p-6">
      <button
        onClick={onBack}
        className="mb-5 inline-flex items-center gap-1.5 font-mono text-xs text-stone-500 transition
          hover:text-stone-800 dark:hover:text-stone-200"
      >
        ← back to dashboard
      </button>

      {/* header card */}
      <div className="relative overflow-hidden rounded-xl border border-stone-200/80 bg-white p-5 shadow-panel
        dark:border-white/[0.06] dark:bg-[#13161b] dark:shadow-panel-dark">
        <span className={`absolute inset-y-0 left-0 w-1 ${s.rail}`} aria-hidden />
        <div className="flex flex-wrap items-center gap-x-4 gap-y-3 pl-2">
          <StatusDot state={state} />
          <h2 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
            {name}
          </h2>
          <span
            className={`rounded-md px-2 py-0.5 font-mono text-[11px] font-medium uppercase tracking-wide ring-1 ring-inset ${s.chip}`}
          >
            {state}
          </span>

          <div className="ml-auto flex gap-2">
            <button
              className="btn-primary"
              onClick={() => withError(api.startWorker(name))}
            >
              ▶ Start
            </button>
            <button className="btn-ghost" onClick={() => withError(api.stopWorker(name))}>
              ■ Stop
            </button>
            <button className="btn-ghost" onClick={() => withError(api.restartWorker(name))}>
              ↻ Restart
            </button>
            <button className="btn-ghost" onClick={() => onEdit?.()}>
              ✎ Edit
            </button>
            {onDelete && (
              <button
                className="rounded-lg border border-rose-500/30 bg-rose-500/5 px-3 py-1.5 font-mono text-xs
                  font-semibold text-rose-600 transition hover:border-rose-500/50 hover:bg-rose-500/10
                  dark:text-rose-300"
                onClick={() => onDelete()}
                aria-label={`Delete ${name}`}
              >
                🗑 Delete
              </button>
            )}
          </div>
        </div>

        {/* metric strip */}
        <div className="mt-5 grid grid-cols-2 gap-3 border-t border-stone-100 pt-4 sm:grid-cols-4 dark:border-white/[0.05]">
          {[
            ["instances up", `${running}/${w.instances.length}`],
            ["restarts", String(restarts)],
            ["concurrency", `×${w.run_mode.concurrency}`],
            ["mode", w.run_mode.type],
          ].map(([label, value]) => (
            <div key={label} className="pl-2">
              <div className="font-mono text-lg font-semibold tabular-nums text-stone-900 dark:text-stone-50">
                {value}
              </div>
              <div className="mt-0.5 text-[10px] uppercase tracking-[0.14em] text-stone-400 dark:text-stone-500">
                {label}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* tabs */}
      <div className="mb-4 mt-6 flex gap-6 border-b border-stone-200/70 dark:border-white/[0.06]">
        <TabBtn id="logs" label="Logs" />
        <TabBtn id="metrics" label="Metrics" />
        <TabBtn id="config" label="Config" />
      </div>

      {tab === "logs" && <LogView worker={name} />}
      {tab === "metrics" && <MetricsTab worker={name} />}
      {tab === "config" && (
        <pre className="scroll-thin overflow-auto rounded-xl border border-white/10 bg-[#0a0c10] p-4
          font-mono text-xs leading-relaxed text-stone-300 shadow-panel-dark">
          {JSON.stringify(w, null, 2)}
        </pre>
      )}
    </div>
  );
}
