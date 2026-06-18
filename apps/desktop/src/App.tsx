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
    <div className="h-screen flex flex-col">
      <div className="flex-1 flex overflow-hidden">
        <Sidebar view={view} setView={setView} />
        <main className="flex-1 overflow-auto">
          {view === "settings" ? (
            <div className="p-6 text-sm opacity-70">Settings coming soon.</div>
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
    <div className="fixed bottom-4 right-4 z-50 max-w-sm flex items-start gap-3 rounded bg-red-600 text-white text-sm px-4 py-3 shadow-lg">
      <span className="flex-1 break-words">{lastError}</span>
      <button
        className="opacity-80 hover:opacity-100 leading-none"
        aria-label="Dismiss error"
        onClick={() => setError(null)}>×</button>
    </div>
  );
}
