import { useStore } from "../store";

export function StatusFooter() {
  const daemon = useStore((s) => s.daemon);
  const ok = daemon?.connected;
  return (
    <div
      className="flex items-center gap-3 border-t border-stone-200/70 bg-white/60 px-5 py-2
        font-mono text-[11px] text-stone-500 backdrop-blur
        dark:border-white/[0.06] dark:bg-[#0c0e12]/80 dark:text-stone-500"
    >
      <span className="flex items-center gap-1.5">
        <span className={`h-1.5 w-1.5 rounded-full ${ok ? "bg-emerald-500" : "bg-rose-500"}`} />
        {ok ? "connected" : "offline"}
      </span>
      {ok && (
        <>
          <span className="text-stone-300 dark:text-stone-700">·</span>
          <span>daemon v{daemon?.version}</span>
          <span className="text-stone-300 dark:text-stone-700">·</span>
          <span className="tabular-nums">{daemon?.running_count} running</span>
          <span className="text-stone-300 dark:text-stone-700">·</span>
          <span className="tabular-nums">{daemon?.worker_count} workers</span>
        </>
      )}
      <span className="ml-auto uppercase tracking-[0.18em] text-stone-300 dark:text-stone-700">
        servicio
      </span>
    </div>
  );
}
