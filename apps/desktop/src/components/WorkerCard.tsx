import type { WorkerStatus } from "../types";
import { worstState, styleFor } from "./status";
import { StatusDot } from "./StatusDot";

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col">
      <span className="font-mono text-sm font-semibold tabular-nums leading-none text-stone-800 dark:text-stone-100">
        {value}
      </span>
      <span className="mt-1 text-[10px] uppercase tracking-[0.14em] text-stone-400 dark:text-stone-500">
        {label}
      </span>
    </div>
  );
}

export function WorkerCard({
  w,
  onOpen,
  onStart,
  onStop,
}: {
  w: WorkerStatus;
  onOpen: () => void;
  onStart: () => void;
  onStop: () => void;
}) {
  const state = worstState(w);
  const s = styleFor(state);
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  const running = w.instances.filter((i) => i.state === "running").length;
  const tags = w.tags ?? [];
  const TAG_CAP = 4;
  const shownTags = tags.slice(0, TAG_CAP);
  const overflow = tags.length - shownTags.length;

  return (
    <div
      onClick={onOpen}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onOpen()}
      className="group relative cursor-pointer overflow-hidden rounded-xl border border-stone-200/80
        bg-white shadow-panel transition duration-200 hover:-translate-y-0.5 hover:border-stone-300
        hover:shadow-lg focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-400
        dark:border-white/[0.06] dark:bg-[#13161b] dark:shadow-panel-dark
        dark:hover:border-white/15"
    >
      {/* status accent rail */}
      <span className={`absolute inset-y-0 left-0 w-1 ${s.rail}`} aria-hidden />

      <div className="p-4 pl-5">
        <div className="flex items-start gap-2.5">
          <StatusDot state={state} />
          <div className="min-w-0 flex-1">
            <h3 className="truncate font-display text-[15px] font-semibold leading-tight text-stone-900 dark:text-stone-50">
              {w.name}
            </h3>
            <p className="mt-0.5 font-mono text-[11px] text-stone-400 dark:text-stone-500">
              daemon · ×{w.run_mode.concurrency}
            </p>
          </div>
          <span
            className={`rounded-md px-2 py-0.5 font-mono text-[11px] font-medium uppercase tracking-wide ring-1 ring-inset ${s.chip}`}
          >
            {state}
          </span>
        </div>

        <div className="mt-4 grid grid-cols-3 gap-2 border-t border-stone-100 pt-3 dark:border-white/[0.05]">
          <Metric label="up" value={`${running}/${w.instances.length}`} />
          <Metric label="restarts" value={String(restarts)} />
          <Metric label="conc" value={`×${w.run_mode.concurrency}`} />
        </div>

        {tags.length > 0 && (
          <div className="mt-3 flex flex-wrap items-center gap-1.5">
            {shownTags.map((t) => (
              <span
                key={t}
                className="rounded font-mono text-[10px] font-medium tracking-wide text-stone-500
                  ring-1 ring-inset ring-stone-300/70 px-1.5 py-0.5
                  dark:text-stone-400 dark:ring-white/10"
              >
                {t}
              </span>
            ))}
            {overflow > 0 && (
              <span
                className="rounded font-mono text-[10px] font-medium tracking-wide text-signal-600
                  ring-1 ring-inset ring-signal-500/30 bg-signal-500/[0.06] px-1.5 py-0.5
                  dark:text-signal-400"
              >
                +{overflow}
              </span>
            )}
          </div>
        )}

        <div
          className="mt-4 flex gap-2 opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="flex-1 rounded-md bg-emerald-500/10 px-2 py-1.5 font-mono text-xs font-semibold
              text-emerald-700 ring-1 ring-inset ring-emerald-500/25 transition hover:bg-emerald-500/20
              dark:text-emerald-300"
            onClick={onStart}
          >
            ▶ start
          </button>
          <button
            className="flex-1 rounded-md bg-stone-500/10 px-2 py-1.5 font-mono text-xs font-semibold
              text-stone-600 ring-1 ring-inset ring-stone-400/25 transition hover:bg-stone-500/20
              dark:text-stone-300"
            onClick={onStop}
          >
            ■ stop
          </button>
        </div>
      </div>
    </div>
  );
}
