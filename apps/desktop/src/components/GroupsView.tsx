import { useMemo, useState } from "react";
import { useStore } from "../store";
import { api } from "../api";
import type { WorkerStatus } from "../types";
import { worstState, styleFor, SIGNAL_OF } from "./status";
import { WorkerCard } from "./WorkerCard";

const UNGROUPED = "Ungrouped";

function groupOf(w: WorkerStatus): string {
  const g = w.group?.trim();
  return g ? g : UNGROUPED;
}

interface GroupBucket {
  name: string;
  workers: WorkerStatus[];
  running: number;
  warming: number;
  down: number;
  idle: number;
}

/** Roll a group's workers into signal-bucket counts (one bucket per worker, by its worst state). */
function rollup(workers: WorkerStatus[]): Omit<GroupBucket, "name" | "workers"> {
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
    <button
      type="button"
      onClick={onOpen}
      className="group/folder relative block w-full text-left focus:outline-none"
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
      </div>
    </button>
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
  const [selected, setSelected] = useState<string | null>(null);

  // group workers; "Ungrouped" sorted last, rest alphabetical
  const groups = useMemo<GroupBucket[]>(() => {
    const byGroup = new Map<string, WorkerStatus[]>();
    for (const w of workers) {
      const g = groupOf(w);
      const list = byGroup.get(g) ?? [];
      list.push(w);
      byGroup.set(g, list);
    }
    return [...byGroup.entries()]
      .sort(([a], [b]) => {
        if (a === UNGROUPED) return 1;
        if (b === UNGROUPED) return -1;
        return a.localeCompare(b);
      })
      .map(([name, list]) => ({ name, workers: list, ...rollup(list) }));
  }, [workers]);

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
    return (
      <div className="flex h-full flex-col">
        <header className="border-b border-stone-200/70 px-6 py-5 dark:border-white/[0.06]">
          <button
            type="button"
            onClick={() => setSelected(null)}
            className="font-mono text-xs text-stone-400 transition hover:text-signal-600 dark:text-stone-500 dark:hover:text-signal-400"
          >
            ← Groups
          </button>
          <div className="mt-2 flex flex-wrap items-baseline gap-3">
            <h1 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
              {active.name}
            </h1>
            <span className="font-mono text-xs text-stone-400 dark:text-stone-500">
              {active.workers.length}{" "}
              {active.workers.length === 1 ? "worker" : "workers"}
            </span>
          </div>
        </header>
        <div className="flex-1 overflow-auto p-6">
          <div className="grid auto-rows-min grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {active.workers.map((w, i) => (
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
