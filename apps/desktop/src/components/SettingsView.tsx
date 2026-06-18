import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { DaemonStatus } from "../types";

interface ServiceState {
  installed: boolean;
  supported?: boolean;
}

const LOG_LINES = 300;
const LOG_POLL_MS = 3000;

export function SettingsView() {
  const [status, setStatus] = useState<ServiceState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updateMsg, setUpdateMsg] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  // Workers config import/export
  const [note, setNote] = useState<string | null>(null);
  const [noteTone, setNoteTone] = useState<"ok" | "err">("ok");
  const [ioBusy, setIoBusy] = useState(false);

  // Debug state
  const [log, setLog] = useState<string | null>(null);
  const [logError, setLogError] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [daemon, setDaemon] = useState<DaemonStatus | null>(null);
  const logBoxRef = useRef<HTMLDivElement>(null);

  const refresh = async () => {
    try {
      setStatus(await api.serviceStatus());
    } catch (e) {
      setError(String(e));
    }
  };

  const refreshLog = async () => {
    try {
      const r = await api.daemonLog(LOG_LINES);
      setLog(r.log);
      setLogError(null);
    } catch {
      setLogError("daemon log unavailable");
    }
  };

  const refreshDiagnostics = async () => {
    try {
      setAppVersion(await api.appVersion());
    } catch {
      setAppVersion(null);
    }
    try {
      setDaemon(await api.daemonStatus());
    } catch {
      setDaemon(null);
    }
  };

  useEffect(() => {
    void refresh();
    void refreshDiagnostics();
    void refreshLog();
    const id = setInterval(() => {
      void refreshLog();
      void refreshDiagnostics();
    }, LOG_POLL_MS);
    return () => clearInterval(id);
  }, []);

  // Auto-scroll the log box to the bottom when new content arrives.
  useEffect(() => {
    const el = logBoxRef.current;
    if (el && typeof el.scrollTo === "function") {
      el.scrollTo(0, el.scrollHeight);
    }
  }, [log]);

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

  const onExport = async () => {
    if (ioBusy) return;
    setIoBusy(true);
    try {
      const p = await api.saveDialog("servicio-workers.json");
      if (p) {
        const n = await api.exportWorkersTo(p);
        setNoteTone("ok");
        setNote(`Exported ${n} ${n === 1 ? "worker" : "workers"} to ${p}`);
      }
    } catch (e) {
      setNoteTone("err");
      setNote(String(e));
    } finally {
      setIoBusy(false);
    }
  };

  const onImport = async () => {
    if (ioBusy) return;
    setIoBusy(true);
    try {
      const p = await api.openFileDialog();
      if (p) {
        const n = await api.importWorkersFrom(p);
        setNoteTone("ok");
        setNote(`Imported ${n} ${n === 1 ? "worker" : "workers"} — they appear on the dashboard.`);
      }
    } catch (e) {
      setNoteTone("err");
      setNote(String(e));
    } finally {
      setIoBusy(false);
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

      {/* Workers config panel */}
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
              portability
            </div>
            <h2 className="mt-1 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
              Workers config
            </h2>
            <p className="mt-1.5 max-w-md text-sm leading-relaxed text-stone-500 dark:text-stone-400">
              Back up or migrate your fleet. Files are a JSON list of worker definitions —
              import merges them onto the dashboard.
            </p>
          </div>

          <div className="flex shrink-0 gap-2">
            <button
              type="button"
              onClick={() => void onExport()}
              disabled={ioBusy}
              className="rounded-lg border border-stone-300/80 bg-white px-3 py-1.5 font-mono
                text-[11px] uppercase tracking-widest text-stone-600 shadow-sm transition-colors
                hover:border-signal-500/60 hover:text-signal-600 disabled:opacity-40
                dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-300 dark:hover:text-signal-400"
            >
              export
            </button>
            <button
              type="button"
              onClick={() => void onImport()}
              disabled={ioBusy}
              className="rounded-lg border border-stone-300/80 bg-white px-3 py-1.5 font-mono
                text-[11px] uppercase tracking-widest text-stone-600 shadow-sm transition-colors
                hover:border-signal-500/60 hover:text-signal-600 disabled:opacity-40
                dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-300 dark:hover:text-signal-400"
            >
              import
            </button>
          </div>
        </div>

        {note && (
          <div
            className={`mt-4 ml-2 rounded-lg border px-3 py-2 font-mono text-[11px] leading-snug ${
              noteTone === "err"
                ? "border-rose-500/30 bg-rose-500/5 text-rose-600 dark:text-rose-300"
                : "border-stone-200/70 bg-stone-50/60 text-stone-600 dark:border-white/[0.06] dark:bg-white/[0.02] dark:text-stone-300"
            }`}
          >
            {note}
          </div>
        )}
      </section>

      {/* Debug panel */}
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
              diagnostics
            </div>
            <h2 className="mt-1 font-display text-lg font-semibold text-stone-900 dark:text-stone-50">
              Debug
            </h2>
            <p className="mt-1.5 max-w-md text-sm leading-relaxed text-stone-500 dark:text-stone-400">
              Live daemon log and runtime diagnostics for troubleshooting.
            </p>
          </div>
        </div>

        {/* diagnostics key/value grid */}
        <dl className="mt-5 grid grid-cols-1 gap-x-8 gap-y-2.5 border-t border-stone-200/70 pl-2 pt-4
          sm:grid-cols-2 dark:border-white/[0.06]">
          <DiagRow label="app version" value={appVersion ?? "—"} />
          <DiagRow
            label="connection"
            value={daemon ? (daemon.connected ? "connected" : "offline") : "—"}
            tone={daemon ? (daemon.connected ? "ok" : "warn") : "muted"}
          />
          <DiagRow label="daemon version" value={daemon?.version || "—"} />
          <DiagRow label="uptime" value={daemon ? fmtUptime(daemon.uptime_secs) : "—"} />
          <DiagRow label="workers" value={daemon ? String(daemon.worker_count) : "—"} />
          <DiagRow label="running" value={daemon ? String(daemon.running_count) : "—"} />
        </dl>

        {/* daemon log viewer — terminal treatment, mirrors LogView */}
        <div className="mt-5 ml-2">
          <div className="mb-2 flex items-center justify-between">
            <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-signal-600 dark:text-signal-400">
              daemon log
            </span>
            <button
              type="button"
              onClick={() => void refreshLog()}
              className="rounded-md border border-stone-300/80 bg-white px-2.5 py-1 font-mono
                text-[10px] uppercase tracking-widest text-stone-600 shadow-sm transition-colors
                hover:border-signal-500/60 hover:text-signal-600
                dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-300 dark:hover:text-signal-400"
            >
              refresh
            </button>
          </div>

          <div className="overflow-hidden rounded-xl border border-white/10 bg-[#0a0c10] shadow-panel-dark">
            {/* terminal chrome */}
            <div className="flex items-center gap-2 border-b border-white/[0.06] bg-white/[0.02] px-4 py-2.5">
              <span className="flex gap-1.5" aria-hidden>
                <span className="h-2.5 w-2.5 rounded-full bg-rose-500/70" />
                <span className="h-2.5 w-2.5 rounded-full bg-amber-400/70" />
                <span className="h-2.5 w-2.5 rounded-full bg-emerald-500/70" />
              </span>
              <span className="ml-1 font-mono text-[11px] text-stone-500">&lt;base&gt;/daemon.log</span>
              <span className="ml-auto font-mono text-[10px] uppercase tracking-widest text-stone-600">
                last {LOG_LINES} lines
              </span>
            </div>

            {/* stream */}
            <div
              ref={logBoxRef}
              data-testid="daemon-log-stream"
              className="scroll-thin h-72 overflow-auto whitespace-pre-wrap break-words px-4 py-2
                font-mono text-xs leading-relaxed text-stone-300"
            >
              {logError ? (
                <span className="text-stone-600">{logError}</span>
              ) : log === null ? (
                <span className="text-stone-600">
                  <span className="text-signal-500">$</span> loading…
                  <span className="ml-1 inline-block h-3.5 w-1.5 translate-y-0.5 animate-pulse bg-stone-600" />
                </span>
              ) : log.trim() === "" ? (
                <span className="text-stone-600">log is empty</span>
              ) : (
                log
              )}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

function DiagRow({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "ok" | "warn" | "muted";
}) {
  const valueColor =
    tone === "ok"
      ? "text-emerald-600 dark:text-emerald-400"
      : tone === "warn"
      ? "text-amber-600 dark:text-amber-400"
      : tone === "muted"
      ? "text-stone-400 dark:text-stone-500"
      : "text-stone-700 dark:text-stone-200";
  return (
    <div className="flex items-baseline justify-between gap-3 border-b border-stone-200/40 py-1 dark:border-white/[0.04]">
      <dt className="font-mono text-[10px] uppercase tracking-widest text-stone-400 dark:text-stone-500">
        {label}
      </dt>
      <dd className={`truncate font-mono text-[11px] tabular-nums ${valueColor}`}>{value}</dd>
    </div>
  );
}

function fmtUptime(secs: number): string {
  if (secs <= 0) return "0s";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const parts: string[] = [];
  if (d) parts.push(`${d}d`);
  if (h) parts.push(`${h}h`);
  if (m) parts.push(`${m}m`);
  if (!d && !h) parts.push(`${s}s`);
  return parts.join(" ");
}
