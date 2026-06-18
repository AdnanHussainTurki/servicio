import { useMemo, useState } from "react";
import { useStore } from "../store";
import { api, withError } from "../api";
import type { WorkerStatus } from "../types";
import { computeGroups, fmtMem, type GroupStat } from "../groupStats";
import { WorkerCard } from "./WorkerCard";
import { DashboardGraphs } from "./DashboardGraphs";

const UNGROUPED = "Ungrouped";

/** A compact instrument readout: value + unit label, mono tabular. */
function StatReadout({ value, label }: { value: string; label: string }) {
  return (
    <span className="inline-flex items-baseline gap-1">
      <span className="font-mono text-xs font-semibold tabular-nums text-stone-700 dark:text-stone-200">
        {value}
      </span>
      <span className="font-mono text-[10px] uppercase tracking-wide text-stone-400 dark:text-stone-500">
        {label}
      </span>
    </span>
  );
}

/** Live aggregate readout for a group section header: mem · cpu · processes. */
function GroupStatStrip({ stat }: { stat: GroupStat | undefined }) {
  if (!stat || stat.processes === 0) return null;
  return (
    <span className="inline-flex items-center gap-3 rounded-full border border-stone-200/70 bg-white/60 px-3 py-1
      shadow-sm dark:border-white/[0.06] dark:bg-white/[0.03]">
      <StatReadout value={fmtMem(stat.mem)} label="mem" />
      <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
      <StatReadout value={`${stat.cpu.toFixed(1)}%`} label="cpu" />
      <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
      <StatReadout value={String(stat.processes)} label={stat.processes === 1 ? "proc" : "procs"} />
    </span>
  );
}

function groupOf(w: WorkerStatus): string {
  const g = w.group?.trim();
  return g ? g : UNGROUPED;
}

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

/** Toggle chip for the tag filter bar. */
function TagFilterChip({
  tag,
  active,
  onToggle,
}: {
  tag: string;
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={active}
      className={
        "rounded-full px-3 py-1 font-mono text-[11px] font-medium tracking-wide ring-1 ring-inset transition " +
        (active
          ? "bg-signal-500 text-white ring-signal-400 shadow-sm"
          : "bg-white/60 text-stone-500 ring-stone-300/70 hover:text-stone-800 hover:ring-stone-400 " +
            "dark:bg-white/[0.03] dark:text-stone-400 dark:ring-white/10 dark:hover:text-stone-200")
      }
    >
      {tag}
    </button>
  );
}

function WorkerGrid({
  workers,
  onOpen,
  onEditWorker,
  onDeleteWorker,
}: {
  workers: WorkerStatus[];
  onOpen: (name: string) => void;
  onEditWorker: (name: string) => void;
  onDeleteWorker?: (name: string) => void;
}) {
  return (
    <div className="grid auto-rows-min grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
      {workers.map((w, i) => (
        <div key={w.name} className="animate-riseIn" style={{ animationDelay: `${i * 40}ms` }}>
          <WorkerCard
            w={w}
            onOpen={() => onOpen(w.name)}
            onStart={() => withError(api.startWorker(w.name))}
            onStop={() => withError(api.stopWorker(w.name))}
            onEdit={() => onEditWorker(w.name)}
            onDelete={onDeleteWorker ? () => onDeleteWorker(w.name) : undefined}
          />
        </div>
      ))}
    </div>
  );
}

function GroupSection({
  name,
  workers,
  stat,
  onOpen,
  onEditWorker,
  onDeleteWorker,
}: {
  name: string;
  workers: WorkerStatus[];
  stat: GroupStat | undefined;
  onOpen: (name: string) => void;
  onEditWorker: (name: string) => void;
  onDeleteWorker?: (name: string) => void;
}) {
  const [open, setOpen] = useState(true);
  return (
    <section>
      <div className="group/sec mb-3 flex items-center gap-2.5">
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
          className="flex items-center gap-2.5 text-left"
        >
          <span
            className={
              "font-mono text-xs text-stone-400 transition-transform dark:text-stone-500 " +
              (open ? "rotate-90" : "rotate-0")
            }
            aria-hidden
          >
            ▸
          </span>
          <h2 className="font-display text-sm font-semibold uppercase tracking-[0.14em] text-stone-600 dark:text-stone-300">
            {name}
          </h2>
          <span className="rounded-full bg-stone-500/10 px-2 py-0.5 font-mono text-[10px] font-semibold tabular-nums text-stone-500 dark:bg-white/[0.05] dark:text-stone-400">
            {workers.length}
          </span>
        </button>
        <GroupStatStrip stat={stat} />
        <span className="ml-1 h-px flex-1 bg-stone-200/70 dark:bg-white/[0.06]" aria-hidden />
      </div>
      {open && <WorkerGrid workers={workers} onOpen={onOpen} onEditWorker={onEditWorker} onDeleteWorker={onDeleteWorker} />}
    </section>
  );
}

export function Dashboard({
  onOpen,
  onAdd,
  onEditWorker,
  onDeleteWorker,
}: {
  onOpen: (name: string) => void;
  onAdd: () => void;
  onEditWorker?: (name: string) => void;
  onDeleteWorker?: (name: string) => void;
}) {
  const workers = Object.values(useStore((s) => s.workers));
  const latestMetric = useStore((s) => s.latestMetric);
  const [activeTags, setActiveTags] = useState<Set<string>>(new Set());

  // live per-group aggregates (mem/cpu/processes) keyed by group name
  const statByGroup = useMemo(() => {
    const stats = computeGroups(workers, latestMetric);
    return new Map(stats.map((s) => [s.group, s]));
  }, [workers, latestMetric]);

  // overall totals across all groups (only meaningful once metrics flow)
  const totals = useMemo(() => {
    let mem = 0,
      cpu = 0,
      procs = 0;
    for (const s of statByGroup.values()) {
      mem += s.mem;
      cpu += s.cpu;
      procs += s.processes;
    }
    return { mem, cpu, procs };
  }, [statByGroup]);

  const running = workers.filter((w) => w.instances.some((i) => i.state === "running")).length;
  const warming = workers.filter((w) =>
    w.instances.some((i) => ["starting", "backoff", "stopping"].includes(i.state))
  ).length;
  // Summary uses "down" (NOT "crashed") to keep the card's "crashed" text unique.
  const down = workers.filter((w) =>
    w.instances.some((i) => i.state === "crashed" || i.state === "failed")
  ).length;

  // distinct tags across all workers, sorted
  const allTags = useMemo(() => {
    const set = new Set<string>();
    for (const w of workers) for (const t of w.tags ?? []) set.add(t);
    return [...set].sort((a, b) => a.localeCompare(b));
  }, [workers]);

  // OR filter: a worker matches if it carries ANY active tag
  const visible = useMemo(() => {
    if (activeTags.size === 0) return workers;
    return workers.filter((w) => (w.tags ?? []).some((t) => activeTags.has(t)));
  }, [workers, activeTags]);

  // group visible workers; "Ungrouped" sorted last, rest alphabetical
  const sections = useMemo(() => {
    const byGroup = new Map<string, WorkerStatus[]>();
    for (const w of visible) {
      const g = groupOf(w);
      const list = byGroup.get(g) ?? [];
      list.push(w);
      byGroup.set(g, list);
    }
    return [...byGroup.entries()].sort(([a], [b]) => {
      if (a === UNGROUPED) return 1;
      if (b === UNGROUPED) return -1;
      return a.localeCompare(b);
    });
  }, [visible]);

  function toggleTag(tag: string) {
    setActiveTags((prev) => {
      const next = new Set(prev);
      if (next.has(tag)) next.delete(tag);
      else next.add(tag);
      return next;
    });
  }

  const filterActive = activeTags.size > 0;

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
          {totals.procs > 0 && (
            <span
              className="inline-flex items-center gap-3 rounded-full border border-stone-200/80 bg-white/70 px-3 py-1
                shadow-sm dark:border-white/[0.07] dark:bg-white/[0.03]"
              title="Total across all groups"
            >
              <StatReadout value={fmtMem(totals.mem)} label="mem" />
              <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
              <StatReadout value={`${totals.cpu.toFixed(1)}%`} label="cpu" />
            </span>
          )}
          <button className="btn-primary ml-1" onClick={onAdd}>
            <span className="text-base leading-none">+</span> New worker
          </button>
        </div>
      </header>

      {workers.length === 0 ? (
        <EmptyState onAdd={onAdd} />
      ) : (
        <div className="flex-1 overflow-auto">
          <DashboardGraphs />
          {allTags.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 border-b border-stone-200/60 px-6 py-3 dark:border-white/[0.05]">
              <span className="mr-1 font-mono text-[10px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
                filter
              </span>
              {allTags.map((t) => (
                <TagFilterChip key={t} tag={t} active={activeTags.has(t)} onToggle={() => toggleTag(t)} />
              ))}
              {filterActive && (
                <button
                  type="button"
                  onClick={() => setActiveTags(new Set())}
                  className="ml-1 font-mono text-[11px] text-stone-500 underline-offset-4 transition hover:text-signal-600 hover:underline dark:hover:text-signal-400"
                >
                  clear
                </button>
              )}
            </div>
          )}

          <div className="space-y-8 p-6">
            {sections.length === 0 ? (
              <p className="rounded-lg border border-dashed border-stone-300 bg-white/40 px-4 py-8 text-center text-sm text-stone-500 dark:border-white/10 dark:bg-white/[0.02] dark:text-stone-400">
                No workers match the active tag filter.
              </p>
            ) : (
              sections.map(([name, list]) => (
                <GroupSection key={name} name={name} workers={list} stat={statByGroup.get(name)} onOpen={onOpen} onEditWorker={onEditWorker ?? (() => {})} onDeleteWorker={onDeleteWorker} />
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
