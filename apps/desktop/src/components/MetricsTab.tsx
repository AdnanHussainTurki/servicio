import { useEffect } from "react";
import { useStore } from "../store";
import { api } from "../api";
import { Sparkline } from "./Sparkline";
import type { MetricPointT } from "../types";

// Stable empty reference — see LogView: `?? []` inside a Zustand selector makes a new array
// each render → React 19 infinite loop. Default outside the selector against this constant.
const EMPTY_POINTS: MetricPointT[] = [];

const CPU_STROKE = "#f97316"; // copper signal
const MEM_STROKE = "#38bdf8"; // telemetry cyan

function fmtMem(bytes: number): string {
  const mb = bytes / 1048576;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb.toFixed(1)} MB`;
}

/** A single instrument readout: label, big mono current value, sparkline. */
function Gauge({
  label,
  value,
  series,
  stroke,
}: {
  label: string;
  value: string;
  series: number[];
  stroke: string;
}) {
  return (
    <div
      className="relative overflow-hidden rounded-xl border border-white/10 bg-[#0a0c10] p-4 shadow-panel-dark"
      style={{ color: stroke }}
    >
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-stone-500">{label}</span>
        <span className="inline-flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-widest text-stone-600">
          <span className="h-1.5 w-1.5 rounded-full" style={{ background: stroke }} />
          live
        </span>
      </div>
      <div className="mb-2 mt-1 font-mono text-3xl font-semibold tabular-nums text-stone-50">{value}</div>
      <Sparkline data={series} stroke={stroke} />
    </div>
  );
}

export function MetricsTab({ worker }: { worker: string }) {
  const points = useStore((s) => s.metrics[worker]) ?? EMPTY_POINTS;
  const applyEvent = useStore((s) => s.applyEvent);

  // Seed the store buffer from history on mount; live `metric` events keep it fresh.
  useEffect(() => {
    let cancelled = false;
    api
      .metrics(worker, 900)
      .then((series) => {
        if (cancelled) return;
        for (const s of series) {
          for (const p of s.points) {
            applyEvent({ kind: "metric", worker, instance: s.instance, ts: p.ts, cpu: p.cpu, mem: p.mem });
          }
        }
      })
      .catch(() => {
        /* history unavailable — fall back to live events only */
      });
    return () => {
      cancelled = true;
    };
  }, [worker, applyEvent]);

  const latest = points.length ? points[points.length - 1] : null;
  const cpuSeries = points.map((p) => p.cpu);
  const memSeries = points.map((p) => p.mem);

  return (
    <div className="animate-riseIn">
      <div className="mb-3 flex items-center justify-between">
        <span className="font-mono text-[11px] uppercase tracking-[0.18em] text-stone-500">
          resource telemetry
        </span>
        <span className="font-mono text-[10px] uppercase tracking-widest text-stone-500">
          window · last 15m
        </span>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <Gauge
          label="cpu"
          value={latest ? `${latest.cpu.toFixed(1)}%` : "—"}
          series={cpuSeries}
          stroke={CPU_STROKE}
        />
        <Gauge
          label="memory"
          value={latest ? fmtMem(latest.mem) : "—"}
          series={memSeries}
          stroke={MEM_STROKE}
        />
      </div>

      {!latest && (
        <div className="mt-3 rounded-xl border border-white/10 bg-[#0a0c10] px-4 py-3 font-mono text-xs text-stone-600 shadow-panel-dark">
          <span className="text-signal-500">$</span> awaiting telemetry…
          <span className="ml-1 inline-block h-3.5 w-1.5 translate-y-0.5 animate-pulse bg-stone-600" />
        </div>
      )}
    </div>
  );
}
