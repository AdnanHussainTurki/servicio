import { useStore } from "../store";
import { api, withError } from "../api";
import { WorkerCard } from "./WorkerCard";

export function Dashboard({ onOpen, onAdd }: { onOpen: (name: string) => void; onAdd: () => void }) {
  const workers = Object.values(useStore((s) => s.workers));
  const running = workers.filter((w) => w.instances.some((i) => i.state === "running")).length;
  const crashed = workers.filter((w) => w.instances.some((i) => i.state === "crashed" || i.state === "failed")).length;
  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex gap-2 text-xs">
          <span className="rounded-full bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-300 px-2 py-0.5">{running} running</span>
          <span className="rounded-full bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-300 px-2 py-0.5">{crashed} down</span>
        </div>
        <button className="rounded bg-blue-600 hover:bg-blue-700 text-white text-sm px-3 py-1.5 transition" onClick={onAdd}>+ New worker</button>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {workers.map((w) => (
          <WorkerCard key={w.name} w={w}
            onOpen={() => onOpen(w.name)}
            onStart={() => withError(api.startWorker(w.name))}
            onStop={() => withError(api.stopWorker(w.name))} />
        ))}
      </div>
    </div>
  );
}
