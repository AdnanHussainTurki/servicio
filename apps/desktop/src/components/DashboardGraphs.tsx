import { useEffect, useMemo, useState } from "react";
import { useStore } from "../store";
import { fmtMem } from "../groupStats";
import { computeTotals } from "../dashboardTotals";
import { Sparkline } from "./Sparkline";

const CPU_STROKE = "#f97316"; // copper signal
const MEM_STROKE = "#38bdf8"; // telemetry cyan
const HISTORY_CAP = 60;

/** A wide instrument panel: label, big current value, area chart. */
function GraphPanel({
  label,
  value,
  series,
  stroke,
  live,
}: {
  label: string;
  value: string;
  series: number[];
  stroke: string;
  live: boolean;
}) {
  return (
    <div
      className="relative overflow-hidden rounded-xl border border-white/10 bg-[#0a0c10] p-4 shadow-panel-dark"
      style={{ color: stroke }}
    >
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-stone-500">{label}</span>
        <span className="inline-flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-widest text-stone-600">
          <span
            className={`h-1.5 w-1.5 rounded-full ${live ? "" : "opacity-30"}`}
            style={{ background: stroke }}
          />
          {live ? "live" : "idle"}
        </span>
      </div>
      <div className="mb-2 mt-1 font-mono text-3xl font-semibold tabular-nums text-stone-50">{value}</div>
      <Sparkline data={series} stroke={stroke} />
    </div>
  );
}

/**
 * A control-room header band with two live aggregate charts — total CPU% and
 * total memory across the whole supervised fleet. Keeps a rolling in-memory
 * history (capped) sampled whenever the computed totals change.
 */
export function DashboardGraphs() {
  const workers = Object.values(useStore((s) => s.workers));
  const latestMetric = useStore((s) => s.latestMetric);

  const totals = useMemo(
    () => computeTotals(workers, latestMetric),
    [workers, latestMetric],
  );

  const [cpuHist, setCpuHist] = useState<number[]>([]);
  const [memHist, setMemHist] = useState<number[]>([]);

  // Append a sample to the rolling history whenever the totals change. We key the
  // effect on the numeric totals (not object identity) so it only fires on real
  // moves. The append is deferred to a microtask so it reads as an async update
  // (sampling an external time series) rather than a synchronous render cascade.
  useEffect(() => {
    if (totals.samples === 0) return;
    let cancelled = false;
    queueMicrotask(() => {
      if (cancelled) return;
      setCpuHist((h) => [...h, totals.cpu].slice(-HISTORY_CAP));
      setMemHist((h) => [...h, totals.mem].slice(-HISTORY_CAP));
    });
    return () => {
      cancelled = true;
    };
  }, [totals.cpu, totals.mem, totals.samples]);

  const hasData = totals.samples > 0;

  return (
    <div className="border-b border-stone-200/70 bg-stone-50/40 px-6 py-5 dark:border-white/[0.06] dark:bg-black/20">
      <div className="mb-3 flex items-center justify-between">
        <span className="font-mono text-[11px] uppercase tracking-[0.18em] text-stone-500">
          fleet telemetry
        </span>
        <span className="font-mono text-[10px] uppercase tracking-widest text-stone-500">
          all workers · live
        </span>
      </div>

      {hasData ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <GraphPanel
            label="total cpu"
            value={`${totals.cpu.toFixed(1)}%`}
            series={cpuHist}
            stroke={CPU_STROKE}
            live
          />
          <GraphPanel
            label="total memory"
            value={fmtMem(totals.mem)}
            series={memHist}
            stroke={MEM_STROKE}
            live
          />
        </div>
      ) : (
        <div className="rounded-xl border border-white/10 bg-[#0a0c10] px-4 py-3 font-mono text-xs text-stone-600 shadow-panel-dark">
          <span className="text-signal-500">$</span> awaiting telemetry…
          <span className="ml-1 inline-block h-3.5 w-1.5 translate-y-0.5 animate-pulse bg-stone-600" />
        </div>
      )}
    </div>
  );
}
