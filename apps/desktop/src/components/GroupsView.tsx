import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../store";
import { api, withError } from "../api";
import type { WorkerStatus } from "../types";
import { computeGroups, fmtMem, type GroupStat } from "../groupStats";
import { worstState, styleFor, SIGNAL_OF } from "./status";
import { Sparkline } from "./Sparkline";
import { WorkerCard } from "./WorkerCard";

const MEM_HISTORY_CAP = 60;

interface GroupBucket {
  name: string;
  workers: WorkerStatus[];
  stat: GroupStat;
  running: number;
  warming: number;
  down: number;
  idle: number;
}

/** Fire start/stop/restart against a whole group; "Ungrouped" maps to the null group on the backend. */
async function startAll(group: string) {
  await withError(api.startGroup(group));
}
async function stopAll(group: string) {
  await withError(api.stopGroup(group));
}
async function restartAll(group: string) {
  await withError(
    (async () => {
      await api.stopGroup(group);
      await api.startGroup(group);
    })()
  );
}

/** Roll a group's workers into signal-bucket counts (one bucket per worker, by its worst state). */
function rollup(workers: WorkerStatus[]): Omit<GroupBucket, "name" | "workers" | "stat"> {
  let running = 0,
    warming = 0,
    down = 0,
    idle = 0;
  for (const w of workers) {
    switch (SIGNAL_OF[worstState(w)]) {
      case "live":
        running++;
        break;
      case "warm":
        warming++;
        break;
      case "down":
        down++;
        break;
      default:
        idle++;
    }
  }
  return { running, warming, down, idle };
}

/** A single rollup count + label, hidden when zero. */
function RollupStat({ count, label, dot }: { count: number; label: string; dot: string }) {
  if (count === 0) return null;
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className={`h-1.5 w-1.5 rounded-full ${dot}`} aria-hidden />
      <span className="font-mono text-xs font-semibold tabular-nums text-stone-700 dark:text-stone-200">
        {count}
      </span>
      <span className="font-mono text-[11px] text-stone-400 dark:text-stone-500">{label}</span>
    </span>
  );
}

/** Inline metric tile: value + unit, mono tabular — used on cards and drill-in. */
function MetricTile({ value, label }: { value: string; label: string }) {
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

/** Live aggregate readout (mem · cpu · procs); hidden until at least one process reports. */
function GroupMetrics({ stat }: { stat: GroupStat }) {
  if (stat.processes === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
      <MetricTile value={fmtMem(stat.mem)} label="mem" />
      <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
      <MetricTile value={`${stat.cpu.toFixed(1)}%`} label="cpu" />
      <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
      <MetricTile value={String(stat.processes)} label={stat.processes === 1 ? "proc" : "procs"} />
    </div>
  );
}

/**
 * Bulk start/stop/restart for a whole group. stopPropagation lets these live on a
 * clickable folder card without triggering drill-in. Stop confirms (it can take a
 * whole group offline). `compact` shrinks them for the folder-card footer.
 */
function BulkActions({ group, count, compact = false }: { group: string; count: number; compact?: boolean }) {
  const stop = (e: React.MouseEvent, fn: () => void) => {
    e.stopPropagation();
    e.preventDefault();
    fn();
  };
  const base =
    "inline-flex items-center gap-1 rounded-md font-mono font-medium ring-1 ring-inset transition " +
    (compact ? "px-2 py-1 text-[10px] " : "px-2.5 py-1.5 text-[11px] ");
  return (
    <div className="flex items-center gap-1.5">
      <button
        type="button"
        aria-label={`Start all in ${group}`}
        title={`Start all in ${group}`}
        onClick={(e) => stop(e, () => startAll(group))}
        className={
          base +
          "bg-emerald-500/10 text-emerald-700 ring-emerald-500/25 hover:bg-emerald-500/20 " +
          "dark:text-emerald-300"
        }
      >
        <span aria-hidden>▶</span> Start all
      </button>
      <button
        type="button"
        aria-label={`Restart all in ${group}`}
        title={`Restart all in ${group}`}
        onClick={(e) => stop(e, () => restartAll(group))}
        className={
          base +
          "bg-amber-400/10 text-amber-700 ring-amber-400/25 hover:bg-amber-400/20 " +
          "dark:text-amber-300"
        }
      >
        <span aria-hidden>↻</span> Restart all
      </button>
      <button
        type="button"
        aria-label={`Stop all in ${group}`}
        title={`Stop all in ${group}`}
        onClick={(e) =>
          stop(e, () => {
            if (window.confirm(`Stop ${count} ${count === 1 ? "worker" : "workers"} in "${group}"?`)) {
              stopAll(group);
            }
          })
        }
        className={
          base +
          "bg-rose-500/10 text-rose-700 ring-rose-500/25 hover:bg-rose-500/20 " +
          "dark:text-rose-300"
        }
      >
        <span aria-hidden>■</span> Stop all
      </button>
    </div>
  );
}

/** The dominant signal of a group drives the folder-tab accent color. */
function tabAccent(g: GroupBucket): string {
  if (g.down > 0) return styleFor("crashed").rail;
  if (g.warming > 0) return styleFor("starting").rail;
  if (g.running > 0) return styleFor("running").rail;
  return styleFor("stopped").rail;
}

function FolderCard({ group, onOpen }: { group: GroupBucket; onOpen: () => void }) {
  const accent = tabAccent(group);
  return (
    // Not a <button> on purpose: it hosts the bulk-action buttons, and nesting
    // interactive buttons is invalid HTML. A role="button" div keeps it clickable
    // + keyboard-accessible without that conflict.
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen();
        }
      }}
      className="group/folder relative block w-full cursor-pointer text-left focus:outline-none"
    >
      {/* protruding folder tab */}
      <span
        className={`ml-4 inline-block h-3 w-16 rounded-t-md border border-b-0 border-stone-200/80
          bg-stone-100 dark:border-white/[0.06] dark:bg-[#13161b]`}
        aria-hidden
      >
        <span className={`block h-[3px] w-full rounded-t-md ${accent}`} />
      </span>

      <div
        className="relative -mt-px overflow-hidden rounded-xl rounded-tl-none border border-stone-200/80
          bg-white p-5 shadow-panel transition duration-200 group-hover/folder:-translate-y-0.5
          group-hover/folder:border-stone-300 group-hover/folder:shadow-lg
          group-focus-visible/folder:ring-2 group-focus-visible/folder:ring-signal-400
          dark:border-white/[0.06] dark:bg-[#13161b] dark:shadow-panel-dark
          dark:group-hover/folder:border-white/15"
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h3 className="truncate font-display text-base font-semibold leading-tight text-stone-900 dark:text-stone-50">
              {group.name}
            </h3>
            <p className="mt-0.5 font-mono text-[11px] text-stone-400 dark:text-stone-500">
              {group.workers.length} {group.workers.length === 1 ? "worker" : "workers"}
            </p>
          </div>
          <span
            className="font-mono text-[10px] uppercase tracking-[0.16em] text-stone-300 transition
              group-hover/folder:text-signal-500 dark:text-stone-600"
            aria-hidden
          >
            open →
          </span>
        </div>

        <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t border-stone-100 pt-3 dark:border-white/[0.05]">
          <RollupStat count={group.running} label="running" dot={styleFor("running").dot} />
          <RollupStat count={group.warming} label="warming" dot={styleFor("starting").dot} />
          <RollupStat count={group.down} label="down" dot={styleFor("crashed").dot} />
          <RollupStat count={group.idle} label="idle" dot={styleFor("stopped").dot} />
        </div>

        {group.stat.processes > 0 && (
          <div className="mt-2.5">
            <GroupMetrics stat={group.stat} />
          </div>
        )}

        <div className="mt-3 border-t border-stone-100 pt-3 dark:border-white/[0.05]">
          <BulkActions group={group.name} count={group.workers.length} compact />
        </div>
      </div>
    </div>
  );
}

function EmptyState({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="flex flex-1 items-center justify-center px-6 py-16">
      <div
        className="animate-riseIn w-full max-w-md rounded-2xl border border-dashed border-stone-300
          bg-white/50 px-8 py-12 text-center shadow-sm backdrop-blur-sm
          dark:border-white/10 dark:bg-white/[0.02]"
      >
        <div
          className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-signal-500/10
            text-2xl text-signal-600 ring-1 ring-inset ring-signal-500/20 dark:text-signal-400"
        >
          ▤
        </div>
        <h2 className="mt-5 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
          No groups yet
        </h2>
        <p className="mx-auto mt-2 max-w-xs text-sm leading-relaxed text-stone-500 dark:text-stone-400">
          Workers you create are grouped by project. Add a worker and it will show up in its
          group folder here.
        </p>
        <button className="btn-primary mt-6" onClick={onAdd}>
          <span className="text-base leading-none">+</span> Add worker
        </button>
      </div>
    </div>
  );
}

export function GroupsView({
  onOpenWorker,
  onAddWorker,
}: {
  onOpenWorker: (name: string) => void;
  onAddWorker: () => void;
}) {
  const workers = Object.values(useStore((s) => s.workers));
  const latestMetric = useStore((s) => s.latestMetric);
  const [selected, setSelected] = useState<string | null>(null);

  // Order + per-group aggregates come from computeGroups (mem desc, Ungrouped last);
  // we layer the signal-bucket rollup on top for the folder-tab accent.
  const groups = useMemo<GroupBucket[]>(
    () =>
      computeGroups(workers, latestMetric).map((stat) => ({
        name: stat.group,
        workers: stat.workers,
        stat,
        ...rollup(stat.workers),
      })),
    [workers, latestMetric]
  );

  if (workers.length === 0) {
    return (
      <div className="flex h-full flex-col">
        <Header subtitle="0 groups" onAdd={onAddWorker} />
        <EmptyState onAdd={onAddWorker} />
      </div>
    );
  }

  const active = selected ? groups.find((g) => g.name === selected) ?? null : null;

  // ── Drill-in: one group's worker cards ──────────────────────────────
  if (active) {
    return <GroupDrillIn group={active} onBack={() => setSelected(null)} onOpenWorker={onOpenWorker} />;
  }

  // ── Default: grid of folder cards ───────────────────────────────────
  return (
    <div className="flex h-full flex-col">
      <Header
        subtitle={`${groups.length} ${groups.length === 1 ? "group" : "groups"}`}
        onAdd={onAddWorker}
      />
      <div className="flex-1 overflow-auto p-6">
        <div className="grid auto-rows-min grid-cols-1 gap-x-5 gap-y-6 sm:grid-cols-2 xl:grid-cols-3">
          {groups.map((g, i) => (
            <div key={g.name} className="animate-riseIn" style={{ animationDelay: `${i * 40}ms` }}>
              <FolderCard group={g} onOpen={() => setSelected(g.name)} />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * Drill-in view for one group: bulk actions, a rolling total-memory sparkline,
 * current mem/cpu/process tiles, then the per-worker cards. The sparkline keeps
 * its own capped history, sampled whenever the group's total memory changes.
 */
function GroupDrillIn({
  group,
  onBack,
  onOpenWorker,
}: {
  group: GroupBucket;
  onBack: () => void;
  onOpenWorker: (name: string) => void;
}) {
  const { stat } = group;
  const [memHistory, setMemHistory] = useState<number[]>([]);
  const lastMem = useRef<number | null>(null);

  // append the current total mem whenever it changes (cap at MEM_HISTORY_CAP)
  useEffect(() => {
    if (stat.processes === 0) return;
    if (lastMem.current === stat.mem) return;
    lastMem.current = stat.mem;
    setMemHistory((h) => {
      const next = [...h, stat.mem];
      if (next.length > MEM_HISTORY_CAP) next.splice(0, next.length - MEM_HISTORY_CAP);
      return next;
    });
  }, [stat.mem, stat.processes]);

  return (
    <div className="flex h-full flex-col">
      <header className="border-b border-stone-200/70 px-6 py-5 dark:border-white/[0.06]">
        <button
          type="button"
          onClick={onBack}
          className="font-mono text-xs text-stone-400 transition hover:text-signal-600 dark:text-stone-500 dark:hover:text-signal-400"
        >
          ← Groups
        </button>
        <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-baseline gap-3">
            <h1 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
              {group.name}
            </h1>
            <span className="font-mono text-xs text-stone-400 dark:text-stone-500">
              {group.workers.length} {group.workers.length === 1 ? "worker" : "workers"}
            </span>
          </div>
          <BulkActions group={group.name} count={group.workers.length} />
        </div>

        {stat.processes > 0 && (
          <div className="mt-4 flex flex-wrap items-center gap-x-6 gap-y-3">
            <div className="text-signal-600 dark:text-signal-400" style={{ minWidth: 180, maxWidth: 280 }}>
              <Sparkline data={memHistory.length ? memHistory : [stat.mem]} stroke="currentColor" />
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
              <MetricTile value={fmtMem(stat.mem)} label="mem" />
              <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
              <MetricTile value={`${stat.cpu.toFixed(1)}%`} label="cpu" />
              <span className="h-3 w-px bg-stone-200 dark:bg-white/10" aria-hidden />
              <MetricTile value={String(stat.processes)} label={stat.processes === 1 ? "proc" : "procs"} />
            </div>
          </div>
        )}
      </header>
      <div className="flex-1 overflow-auto p-6">
        <div className="grid auto-rows-min grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {group.workers.map((w, i) => (
            <div key={w.name} className="animate-riseIn" style={{ animationDelay: `${i * 40}ms` }}>
              <WorkerCard
                w={w}
                onOpen={() => onOpenWorker(w.name)}
                onStart={() => void api.startWorker(w.name)}
                onStop={() => void api.stopWorker(w.name)}
              />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function Header({ subtitle, onAdd }: { subtitle: string; onAdd: () => void }) {
  return (
    <header
      className="flex flex-wrap items-center justify-between gap-4 border-b border-stone-200/70
        px-6 py-5 dark:border-white/[0.06]"
    >
      <div>
        <h1 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
          Groups
        </h1>
        <p className="mt-1 font-mono text-xs text-stone-400 dark:text-stone-500">{subtitle}</p>
      </div>
      <button className="btn-primary" onClick={onAdd}>
        <span className="text-base leading-none">+</span> Add worker
      </button>
    </header>
  );
}
