import { useEffect, useState } from "react";
import { useStore } from "./store";
import { api, subscribeEvents } from "./api";
import { Sidebar } from "./components/Sidebar";
import { StatusFooter } from "./components/StatusFooter";
import { Dashboard } from "./components/Dashboard";
import { GroupsView } from "./components/GroupsView";
import { WorkerDetail } from "./components/WorkerDetail";
import { CreateFlow } from "./components/CreateFlow";
import type { EditSpec } from "./components/CreateFlow";
import { SettingsView } from "./components/SettingsView";

export default function App() {
  const [view, setView] = useState("dashboard");
  const [detail, setDetail] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [editSpec, setEditSpec] = useState<EditSpec | null>(null);

  // When an edit is requested, fetch the full spec from the daemon. Guarded so a
  // failed fetch (or dev browser without Tauri) never crashes the app.
  useEffect(() => {
    if (!editing) {
      setEditSpec(null);
      return;
    }
    let alive = true;
    (async () => {
      try {
        const spec = await api.getWorker(editing);
        if (alive) setEditSpec(spec as EditSpec);
      } catch (err) {
        if (alive) {
          useStore.getState().setError(`Could not load "${editing}" for editing: ${String(err)}`);
          setEditing(null);
        }
      }
    })();
    return () => { alive = false; };
  }, [editing]);

  function closeFlow() {
    setAdding(false);
    setEditing(null);
    setEditSpec(null);
  }

  useEffect(() => {
    subscribeEvents().catch(() => {});
    let alive = true;
    let appVer: string | null = null;
    const tick = async () => {
      try {
        const d = await api.daemonStatus();
        useStore.getState().setDaemon(d);
        useStore.getState().setWorkers(await api.listWorkers());
        // Version-skew guard: warn when the connected daemon's version differs
        // from the app's expected version (stale daemon → missing methods).
        try {
          if (appVer === null) appVer = await api.appVersion();
          const dv = d.version;
          if (appVer && dv && dv !== appVer) {
            useStore.getState().setDaemonWarning(
              `Daemon version ${dv} is running but this app expects ${appVer}. ` +
              `Quit all Servicio instances (or run: pkill servicio-daemon) and reopen.`,
            );
          } else {
            useStore.getState().setDaemonWarning(null);
          }
        } catch { /* version check is best-effort; never crash the poll */ }
      } catch { /* daemon not ready yet */ }
      if (alive) setTimeout(tick, 2000);
    };
    tick();
    return () => { alive = false; };
  }, []);

  return (
    <div className="flex h-screen flex-col">
      <DaemonWarningBanner />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar view={view} setView={setView} />
        <main className="scroll-thin flex-1 overflow-auto">
          {view === "settings" ? (
            <SettingsView />
          ) : adding ? (
            <CreateFlow onDone={closeFlow} onCancel={closeFlow} />
          ) : editing ? (
            editSpec ? (
              <CreateFlow editWorker={editSpec} onDone={closeFlow} onCancel={closeFlow} />
            ) : (
              <div className="p-8 font-mono text-sm text-stone-400 dark:text-stone-500">
                Loading {editing}…
              </div>
            )
          ) : detail ? (
            <WorkerDetail name={detail} onBack={() => setDetail(null)} onEdit={() => setEditing(detail)} />
          ) : view === "groups" ? (
            <GroupsView onOpenWorker={setDetail} onAddWorker={() => setAdding(true)} />
          ) : (
            <Dashboard
              onOpen={setDetail}
              onAdd={() => setAdding(true)}
              onEditWorker={(name) => setEditing(name)}
            />
          )}
        </main>
      </div>
      <StatusFooter />
      <ErrorToast />
    </div>
  );
}

function DaemonWarningBanner() {
  const warning = useStore((s) => s.daemonWarning);
  const setWarning = useStore((s) => s.setDaemonWarning);
  if (!warning) return null;
  return (
    <div
      role="alert"
      className="relative flex items-start gap-3 border-b border-amber-500/30 bg-amber-500/[0.08]
        px-4 py-2.5 text-xs text-amber-800 dark:bg-amber-400/[0.06] dark:text-amber-200"
    >
      <span className="absolute inset-y-0 left-0 w-1 bg-amber-500" aria-hidden />
      <span
        className="mt-px flex h-4 w-4 shrink-0 items-center justify-center rounded-full
          bg-amber-500/20 font-mono text-[10px] font-bold text-amber-700 dark:text-amber-300"
        aria-hidden
      >
        !
      </span>
      <div className="min-w-0 flex-1 pl-0.5">
        <span className="font-mono text-[10px] uppercase tracking-[0.16em] text-amber-600 dark:text-amber-400">
          version skew
        </span>
        <span className="ml-2 break-words leading-snug">{warning}</span>
      </div>
      <button
        className="shrink-0 rounded-md px-1.5 text-base leading-none text-amber-600/70 transition
          hover:bg-amber-500/10 hover:text-amber-800 dark:text-amber-300/70 dark:hover:text-amber-100"
        aria-label="Dismiss warning"
        onClick={() => setWarning(null)}
      >
        ×
      </button>
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
