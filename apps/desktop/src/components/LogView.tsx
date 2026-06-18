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
    <div ref={ref} className="h-80 overflow-auto rounded-lg bg-slate-950 text-slate-200 font-mono text-xs p-3">
      {lines.length === 0 ? <div className="opacity-50">No logs yet…</div>
        : lines.map((l, i) => <div key={i} className="whitespace-pre-wrap">{l}</div>)}
    </div>
  );
}
