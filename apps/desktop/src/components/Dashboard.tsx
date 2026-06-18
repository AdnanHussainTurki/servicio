import { useStore } from "../store";
import { api, withError } from "../api";
import { WorkerCard } from "./WorkerCard";

function SummaryChip({
  tone,
  count,
  label,
}: {
  tone: "live" | "warm" | "down";
  count: number;
  label: string;
}) {
  const dot =
    tone === "live" ? "bg-emerald-500" : tone === "warm" ? "bg-amber-400" : "bg-rose-500";
  return (
    <span
      className="inline-flex items-center gap-2 rounded-full border border-stone-200/80 bg-white/70 px-3 py-1
        text-xs font-medium text-stone-600 shadow-sm dark:border-white/[0.07] dark:bg-white/[0.03] dark:text-stone-300"
    >
      <span className={`h-1.5 w-1.5 rounded-full ${dot}`} />
      <span className="font-mono font-semibold tabular-nums text-stone-900 dark:text-stone-100">
        {count}
      </span>
      <span className="text-stone-400 dark:text-stone-500">{label}</span>
    </span>
  );
}

function EmptyState({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="flex flex-1 items-center justify-center px-6 py-16">
      <div className="animate-riseIn w-full max-w-md rounded-2xl border border-dashed border-stone-300
        bg-white/50 px-8 py-12 text-center shadow-sm backdrop-blur-sm
        dark:border-white/10 dark:bg-white/[0.02]">
        <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-signal-500/10
          text-2xl text-signal-600 ring-1 ring-inset ring-signal-500/20 dark:text-signal-400">
          ◇
        </div>
        <h2 className="mt-5 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
          No workers yet
        </h2>
        <p className="mx-auto mt-2 max-w-xs text-sm leading-relaxed text-stone-500 dark:text-stone-400">
          Servicio supervises your long-running processes. Add one and it will keep it
          alive, restart on crash, and stream its logs here.
        </p>
        <button className="btn-primary mt-6" onClick={onAdd}>
          <span className="text-base leading-none">+</span> Add your first worker
        </button>
      </div>
    </div>
  );
}

export function Dashboard({
  onOpen,
  onAdd,
}: {
  onOpen: (name: string) => void;
  onAdd: () => void;
}) {
  const workers = Object.values(useStore((s) => s.workers));
  const running = workers.filter((w) => w.instances.some((i) => i.state === "running")).length;
  const warming = workers.filter((w) =>
    w.instances.some((i) => ["starting", "backoff", "stopping"].includes(i.state))
  ).length;
  // Summary uses "down" (NOT "crashed") to keep the card's "crashed" text unique.
  const down = workers.filter((w) =>
    w.instances.some((i) => i.state === "crashed" || i.state === "failed")
  ).length;

  return (
    <div className="flex h-full flex-col">
      <header className="flex flex-wrap items-center justify-between gap-4 border-b border-stone-200/70
        px-6 py-5 dark:border-white/[0.06]">
        <div>
          <h1 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
            Workers
          </h1>
          <p className="mt-1 font-mono text-xs text-stone-400 dark:text-stone-500">
            {workers.length} supervised {workers.length === 1 ? "process" : "processes"}
          </p>
        </div>
        <div className="flex items-center gap-2.5">
          <SummaryChip tone="live" count={running} label="running" />
          <SummaryChip tone="warm" count={warming} label="warming" />
          <SummaryChip tone="down" count={down} label="down" />
          <button className="btn-primary ml-1" onClick={onAdd}>
            <span className="text-base leading-none">+</span> New worker
          </button>
        </div>
      </header>

      {workers.length === 0 ? (
        <EmptyState onAdd={onAdd} />
      ) : (
        <div className="grid auto-rows-min grid-cols-1 gap-4 p-6 sm:grid-cols-2 xl:grid-cols-3">
          {workers.map((w, i) => (
            <div key={w.name} className="animate-riseIn" style={{ animationDelay: `${i * 40}ms` }}>
              <WorkerCard
                w={w}
                onOpen={() => onOpen(w.name)}
                onStart={() => withError(api.startWorker(w.name))}
                onStop={() => withError(api.stopWorker(w.name))}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
