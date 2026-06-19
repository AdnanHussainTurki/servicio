import { useState } from "react";
import { api } from "../api";
import { useStore } from "../store";
import { useTheme } from "../theme";

function NavItem({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: string;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={`group relative flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition
        ${
          active
            ? "bg-white/10 text-white"
            : "text-stone-400 hover:bg-white/[0.06] hover:text-stone-200"
        }`}
    >
      {/* active marker */}
      <span
        className={`absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-signal-500 transition
          ${active ? "opacity-100" : "opacity-0"}`}
        aria-hidden
      />
      <span className="w-4 text-center text-base leading-none opacity-90">{icon}</span>
      <span>{label}</span>
    </button>
  );
}

export function Sidebar({
  view,
  setView,
}: {
  view: string;
  setView: (v: string) => void;
}) {
  const daemon = useStore((s) => s.daemon);
  const setDaemon = useStore((s) => s.setDaemon);
  const ok = daemon?.connected;
  const running = daemon?.running_count ?? 0;
  const { theme, toggle } = useTheme();

  const [confirmStop, setConfirmStop] = useState(false);
  const [busy, setBusy] = useState<null | "stopping" | "starting">(null);

  const doStop = async () => {
    setBusy("stopping");
    setConfirmStop(false);
    try {
      await api.stopDaemon();
      // Reflect the stopped state immediately; the poll keeps it in sync.
      setDaemon({ connected: false, version: "", uptime_secs: 0, worker_count: 0, running_count: 0 });
    } catch (e) {
      useStore.getState().setDaemonWarning(`Failed to stop daemon: ${String(e)}`);
    } finally {
      setBusy(null);
    }
  };

  const doStart = async () => {
    setBusy("starting");
    try {
      await api.startDaemon();
      setDaemon(await api.daemonStatus());
    } catch (e) {
      useStore.getState().setDaemonWarning(`Failed to start daemon: ${String(e)}`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <nav className="flex w-56 shrink-0 flex-col bg-[#0c0e12] text-stone-300">
      {/* brand */}
      <div className="flex items-center gap-2.5 px-5 pb-5 pt-6">
        <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-signal-500
          font-display text-lg font-bold text-white shadow-[0_0_18px_-2px_rgba(249,115,22,0.6)]">
          S
        </span>
        <div className="leading-tight">
          <div className="font-display text-[15px] font-bold tracking-tight text-white">
            Servicio
          </div>
          <div className="font-mono text-[10px] uppercase tracking-[0.2em] text-stone-500">
            supervisor
          </div>
        </div>
      </div>

      {/* nav */}
      <div className="flex flex-col gap-1 px-3">
        <NavItem
          active={view === "dashboard"}
          onClick={() => setView("dashboard")}
          icon="▦"
          label="Dashboard"
        />
        <NavItem
          active={view === "groups"}
          onClick={() => setView("groups")}
          icon="▤"
          label="Groups"
        />
        <NavItem
          active={view === "settings"}
          onClick={() => setView("settings")}
          icon="⚙"
          label="Settings"
        />
      </div>

      <div className="flex-1" />

      {/* daemon status + controls */}
      <div className="mx-3 mb-3 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${
              ok ? "animate-pulseDot bg-emerald-500" : "bg-rose-500"
            }`}
            style={{ "--dot": "rgba(16,185,129,0.55)" } as React.CSSProperties}
          />
          <span className="font-mono text-[11px] font-medium text-stone-300">
            {busy === "stopping"
              ? "stopping…"
              : busy === "starting"
                ? "starting…"
                : ok
                  ? "daemon online"
                  : "daemon stopped"}
          </span>
        </div>
        {ok && (
          <div className="mt-1 pl-4 font-mono text-[10px] text-stone-500">
            v{daemon?.version} · {running} running
          </div>
        )}

        {/* start / stop control */}
        {ok ? (
          <button
            onClick={() => setConfirmStop(true)}
            disabled={busy !== null}
            className="mt-2.5 flex w-full items-center justify-center gap-1.5 rounded-md border border-rose-500/30
              bg-rose-500/10 px-2 py-1.5 font-mono text-[11px] font-medium text-rose-300 transition
              hover:bg-rose-500/20 disabled:opacity-50"
          >
            <span className="text-[9px] leading-none">■</span> Stop daemon
          </button>
        ) : (
          <button
            onClick={doStart}
            disabled={busy !== null}
            className="mt-2.5 flex w-full items-center justify-center gap-1.5 rounded-md border border-emerald-500/30
              bg-emerald-500/10 px-2 py-1.5 font-mono text-[11px] font-medium text-emerald-300 transition
              hover:bg-emerald-500/20 disabled:opacity-50"
          >
            <span className="text-[9px] leading-none">▶</span> Start daemon
          </button>
        )}
      </div>

      {/* theme toggle */}
      <button
        onClick={toggle}
        className="mx-3 mb-5 flex items-center justify-between rounded-lg border border-white/[0.06]
          bg-white/[0.02] px-3 py-2 text-sm text-stone-400 transition hover:bg-white/[0.06] hover:text-stone-200"
        aria-label="Toggle color theme"
      >
        <span className="flex items-center gap-2.5">
          <span className="w-4 text-center text-base leading-none">
            {theme === "dark" ? "☾" : "☀"}
          </span>
          <span className="font-medium">{theme === "dark" ? "Dark" : "Light"}</span>
        </span>
        <span className="font-mono text-[10px] uppercase tracking-widest text-stone-600">
          {theme === "dark" ? "on" : "off"}
        </span>
      </button>

      {/* stop confirmation modal */}
      {confirmStop && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
          onClick={() => setConfirmStop(false)}
        >
          <div
            className="w-full max-w-md rounded-xl border border-white/10 bg-[#14171d] p-5 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-2">
              <span className="text-lg text-rose-400">⚠</span>
              <h2 className="font-display text-base font-bold text-white">Stop the daemon?</h2>
            </div>
            <div className="mt-3 space-y-2 text-sm text-stone-300">
              <p>
                This stops <span className="font-semibold text-white">all {running} running worker{running === 1 ? "" : "s"}</span> and
                ends supervision. While stopped:
              </p>
              <ul className="list-disc space-y-1 pl-5 text-stone-400">
                <li>workers won't be restarted on crash;</li>
                <li>scheduled and batch jobs won't run;</li>
                <li>start-on-login is suspended until you start the daemon again.</li>
              </ul>
              <p className="text-stone-400">
                You can restart it any time from this sidebar.
              </p>
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button
                onClick={() => setConfirmStop(false)}
                className="rounded-md border border-white/10 px-3 py-1.5 text-sm text-stone-300 transition hover:bg-white/[0.06]"
              >
                Cancel
              </button>
              <button
                onClick={doStop}
                className="rounded-md bg-rose-500 px-3 py-1.5 text-sm font-medium text-white transition hover:bg-rose-600"
              >
                Stop daemon
              </button>
            </div>
          </div>
        </div>
      )}
    </nav>
  );
}
