import { useEffect, useState } from "react";
import { useStore } from "./store";
import { api, subscribeEvents, withError } from "./api";
import { Sidebar } from "./components/Sidebar";
import { StatusFooter } from "./components/StatusFooter";
import { Dashboard } from "./components/Dashboard";
import { WorkerDetail } from "./components/WorkerDetail";
import { AddWorkerForm } from "./components/AddWorkerForm";

export default function App() {
  const [view, setView] = useState("dashboard");
  const [detail, setDetail] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    subscribeEvents().catch(() => {});
    let alive = true;
    const tick = async () => {
      try {
        const d = await api.daemonStatus();
        useStore.getState().setDaemon(d);
        useStore.getState().setWorkers(await api.listWorkers());
      } catch { /* daemon not ready yet */ }
      if (alive) setTimeout(tick, 2000);
    };
    tick();
    return () => { alive = false; };
  }, []);

  return (
    <div className="flex h-screen flex-col">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar view={view} setView={setView} />
        <main className="scroll-thin flex-1 overflow-auto">
          {view === "settings" ? (
            <div className="mx-auto max-w-2xl p-6">
              <h1 className="font-display text-2xl font-bold tracking-tight">Settings</h1>
              <p className="mt-2 text-sm text-stone-500 dark:text-stone-400">
                Settings coming soon.
              </p>
            </div>
          ) : adding ? (
            <AddWorkerForm
              onSubmit={async (spec) => {
                const ok = await withError(api.addWorker(spec).then(() => true));
                if (ok) setAdding(false);
              }}
              onCancel={() => setAdding(false)} />
          ) : detail ? (
            <WorkerDetail name={detail} onBack={() => setDetail(null)} />
          ) : (
            <Dashboard onOpen={setDetail} onAdd={() => setAdding(true)} />
          )}
        </main>
      </div>
      <StatusFooter />
      <ErrorToast />
    </div>
  );
}

function ErrorToast() {
  const lastError = useStore((s) => s.lastError);
  const setError = useStore((s) => s.setError);
  if (!lastError) return null;
  return (
    <div className="animate-toastIn fixed bottom-5 right-5 z-50 flex max-w-sm items-start gap-3
      overflow-hidden rounded-xl border border-rose-500/30 bg-[#1a1012]/95 px-4 py-3.5
      text-sm text-rose-50 shadow-[0_12px_40px_-12px_rgba(244,63,94,0.5)] backdrop-blur">
      <span className="absolute inset-y-0 left-0 w-1 bg-rose-500" aria-hidden />
      <span
        className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full
          bg-rose-500/20 text-xs font-bold text-rose-300"
        aria-hidden
      >
        !
      </span>
      <div className="min-w-0 flex-1 pl-1">
        <div className="font-mono text-[10px] uppercase tracking-[0.16em] text-rose-400">
          error
        </div>
        <span className="mt-0.5 block break-words leading-snug text-rose-100">{lastError}</span>
      </div>
      <button
        className="shrink-0 rounded-md px-1.5 text-lg leading-none text-rose-300/70 transition hover:bg-white/10 hover:text-rose-100"
        aria-label="Dismiss error"
        onClick={() => setError(null)}
      >
        ×
      </button>
    </div>
  );
}
