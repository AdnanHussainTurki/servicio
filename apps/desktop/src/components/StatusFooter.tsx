import { useStore } from "../store";

export function StatusFooter() {
  const daemon = useStore((s) => s.daemon);
  const ok = daemon?.connected;
  return (
    <div className="flex items-center gap-2 px-3 py-2 text-xs border-t border-slate-200 dark:border-slate-800">
      <span className={`h-2 w-2 rounded-full ${ok ? "bg-green-500" : "bg-red-500"}`} />
      <span>{ok ? `daemon ${daemon?.version} · ${daemon?.running_count} running` : "daemon not connected"}</span>
    </div>
  );
}
