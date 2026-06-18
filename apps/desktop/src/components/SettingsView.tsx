import { useEffect, useState } from "react";
import { api } from "../api";

interface ServiceState {
  installed: boolean;
  supported?: boolean;
}

export function SettingsView() {
  const [status, setStatus] = useState<ServiceState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updateMsg, setUpdateMsg] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const refresh = async () => {
    try {
      setStatus(await api.serviceStatus());
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const onCheckUpdate = async () => {
    if (checking) return;
    setChecking(true);
    setUpdateMsg(null);
    try {
      setUpdateMsg(await api.checkUpdate());
    } catch (e) {
      setUpdateMsg(String(e));
    } finally {
      setChecking(false);
    }
  };

  const supported = status?.supported !== false;
  const installed = status?.installed ?? false;

  const onToggle = async (next: boolean) => {
    if (busy || !supported) return;
    setBusy(true);
    setError(null);
    try {
      if (next) await api.installService();
      else await api.uninstallService();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-2xl p-6">
      <header className="mb-6">
        <h1 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
          Settings
        </h1>
        <p className="mt-1 font-mono text-xs text-stone-400 dark:text-stone-500">
          system service · login persistence
        </p>
      </header>

      {/* Service panel */}
      <section
        className="relative overflow-hidden rounded-2xl border border-stone-200/80 bg-white/70 p-5 shadow-sm
          backdrop-blur-sm dark:border-white/[0.07] dark:bg-white/[0.02]"
      >
        {/* copper edge accent */}
        <span
          className="absolute inset-y-0 left-0 w-[3px] bg-signal-500/70"
          aria-hidden
        />

        <div className="flex items-start justify-between gap-4 pl-2">
          <div className="min-w-0">
            <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-signal-600 dark:text-signal-400">
              login service
            </div>
            <h2 className="mt-1 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
              Start on login
            </h2>
            <p className="mt-1.5 max-w-md text-sm leading-relaxed text-stone-500 dark:text-stone-400">
              Runs the daemon at login so workers survive reboot.
            </p>
          </div>

          {/* toggle */}
          <label className="flex shrink-0 cursor-pointer select-none items-center gap-3">
            <span className="font-mono text-[10px] uppercase tracking-widest text-stone-400 dark:text-stone-500">
              {installed ? "on" : "off"}
            </span>
            <span className="relative inline-flex">
              <input
                type="checkbox"
                aria-label="Start on login"
                className="peer sr-only"
                checked={installed}
                disabled={busy || !supported}
                onChange={(e) => onToggle(e.target.checked)}
              />
              <span
                className="h-6 w-11 rounded-full bg-stone-300 transition-colors
                  peer-checked:bg-signal-500 peer-disabled:opacity-40
                  dark:bg-white/10 dark:peer-checked:bg-signal-500"
                aria-hidden
              />
              <span
                className="pointer-events-none absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow
                  transition-transform peer-checked:translate-x-5"
                aria-hidden
              />
            </span>
          </label>
        </div>

        {/* status line */}
        <div className="mt-5 flex items-center gap-2.5 border-t border-stone-200/70 pl-2 pt-4 dark:border-white/[0.06]">
          <span
            className={`h-2 w-2 rounded-full ${
              !supported
                ? "bg-stone-400"
                : installed
                ? "bg-emerald-500"
                : "bg-stone-400 dark:bg-stone-600"
            }`}
            aria-hidden
          />
          <span className="font-mono text-[11px] text-stone-500 dark:text-stone-400">
            {!supported
              ? "not supported on this OS"
              : status === null
              ? "checking…"
              : installed
              ? "Service installed"
              : "Service not installed"}
          </span>
        </div>

        {error && (
          <div className="mt-3 ml-2 rounded-lg border border-rose-500/30 bg-rose-500/5 px-3 py-2
            font-mono text-[11px] leading-snug text-rose-600 dark:text-rose-300">
            {error}
          </div>
        )}
      </section>

      {/* Updates panel */}
      <section
        className="relative mt-5 overflow-hidden rounded-2xl border border-stone-200/80 bg-white/70 p-5 shadow-sm
          backdrop-blur-sm dark:border-white/[0.07] dark:bg-white/[0.02]"
      >
        <span
          className="absolute inset-y-0 left-0 w-[3px] bg-signal-500/70"
          aria-hidden
        />

        <div className="flex items-start justify-between gap-4 pl-2">
          <div className="min-w-0">
            <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-signal-600 dark:text-signal-400">
              software updates
            </div>
            <h2 className="mt-1 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
              Check for updates
            </h2>
            <p className="mt-1.5 max-w-md text-sm leading-relaxed text-stone-500 dark:text-stone-400">
              Ask the update server whether a newer Servicio build is available.
            </p>
          </div>

          <button
            type="button"
            onClick={onCheckUpdate}
            disabled={checking}
            className="shrink-0 rounded-lg border border-stone-300/80 bg-white px-3 py-1.5 font-mono
              text-[11px] uppercase tracking-widest text-stone-600 shadow-sm transition-colors
              hover:border-signal-500/60 hover:text-signal-600 disabled:opacity-40
              dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-300 dark:hover:text-signal-400"
          >
            {checking ? "checking…" : "check"}
          </button>
        </div>

        {updateMsg && (
          <div className="mt-4 ml-2 rounded-lg border border-stone-200/70 bg-stone-50/60 px-3 py-2
            font-mono text-[11px] leading-snug text-stone-600 dark:border-white/[0.06] dark:bg-white/[0.02] dark:text-stone-300">
            {updateMsg}
          </div>
        )}
      </section>
    </div>
  );
}
