import { useEffect, useState } from "react";
import { useStore } from "./store";
import { api, subscribeEvents } from "./api";
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
    subscribeEvents();
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
              onSubmit={async (spec) => { await api.addWorker(spec); setAdding(false); }}
              onCancel={() => setAdding(false)} />
          ) : detail ? (
            <WorkerDetail name={detail} onBack={() => setDetail(null)} />
          ) : (
            <Dashboard onOpen={setDetail} onAdd={() => setAdding(true)} />
          )}
        </main>
      </div>
      <StatusFooter />
    </div>
  );
}
