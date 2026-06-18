import { useEffect, useRef } from "react";
import { useStore } from "../store";

export function LogView({ worker }: { worker: string }) {
  const lines = useStore((s) => s.logs[worker] ?? []);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (ref.current && typeof ref.current.scrollTo === "function") {
      ref.current.scrollTo(0, ref.current.scrollHeight);
    }
  }, [lines.length]);

  return (
    <div className="overflow-hidden rounded-xl border border-white/10 bg-[#0a0c10] shadow-panel-dark">
      {/* terminal chrome */}
      <div className="flex items-center gap-2 border-b border-white/[0.06] bg-white/[0.02] px-4 py-2.5">
        <span className="flex gap-1.5" aria-hidden>
          <span className="h-2.5 w-2.5 rounded-full bg-rose-500/70" />
          <span className="h-2.5 w-2.5 rounded-full bg-amber-400/70" />
          <span className="h-2.5 w-2.5 rounded-full bg-emerald-500/70" />
        </span>
        <span className="ml-1 font-mono text-[11px] text-stone-500">{worker} — stdout/stderr</span>
        <span className="ml-auto font-mono text-[10px] uppercase tracking-widest text-stone-600">
          {lines.length} lines
        </span>
      </div>

      {/* stream */}
      <div
        ref={ref}
        className="scroll-thin h-80 overflow-auto px-0 py-2 font-mono text-xs leading-relaxed text-stone-300"
      >
        {lines.length === 0 ? (
          <div className="px-4 py-2 text-stone-600">
            <span className="text-signal-500">$</span> waiting for output…
            <span className="ml-1 inline-block h-3.5 w-1.5 translate-y-0.5 animate-pulse bg-stone-600" />
          </div>
        ) : (
          lines.map((l, i) => {
            const isErr = l.startsWith("[stderr]");
            return (
              <div
                key={i}
                className={`group flex gap-3 px-4 py-px transition hover:bg-white/[0.03] ${
                  isErr ? "text-rose-300/90" : ""
                }`}
              >
                <span className="w-8 shrink-0 select-none text-right text-[10px] text-stone-700 tabular-nums">
                  {i + 1}
                </span>
                <span className="min-w-0 whitespace-pre-wrap break-words">{l}</span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
