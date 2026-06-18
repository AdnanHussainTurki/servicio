import { useState } from "react";
import { api } from "../api";
import type { AddWorkerSpec } from "../api";
import type { SuggestionDraft, RunModeAny } from "../types";

/* ── shared input styling, mirrored from AddWorkerForm ───────────────────── */
const fieldCls =
  "w-full rounded-md border border-stone-300 bg-white px-3 py-2 font-mono text-sm text-stone-900 " +
  "shadow-sm transition placeholder:text-stone-400 focus:border-signal-400 focus:outline-none " +
  "focus:ring-2 focus:ring-signal-400/30 dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-100 " +
  "dark:placeholder:text-stone-600";

const panelCls =
  "relative overflow-hidden rounded-xl border border-stone-200/80 bg-white shadow-panel " +
  "dark:border-white/[0.06] dark:bg-[#13161b] dark:shadow-panel-dark";

function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label
        htmlFor={id}
        className="mb-1.5 block text-xs font-medium text-stone-600 dark:text-stone-400"
      >
        {label}
        {hint && (
          <span className="ml-2 font-mono text-[10px] font-normal text-stone-400 dark:text-stone-500">
            {hint}
          </span>
        )}
      </label>
      {children}
    </div>
  );
}

/* ── run-mode chip, echoing the status-chip ring pattern ─────────────────── */
const MODE_CHIP: Record<RunModeAny["type"], string> = {
  daemon:
    "bg-emerald-500/10 text-emerald-700 ring-emerald-500/25 dark:text-emerald-300",
  scheduled:
    "bg-sky-500/10 text-sky-700 ring-sky-500/25 dark:text-sky-300",
  batch:
    "bg-violet-500/10 text-violet-700 ring-violet-500/25 dark:text-violet-300",
};

function ModeChip({ type }: { type: RunModeAny["type"] }) {
  return (
    <span
      className={`rounded-md px-2 py-0.5 font-mono text-[10px] font-medium uppercase tracking-wide ring-1 ring-inset ${MODE_CHIP[type]}`}
    >
      {type}
    </span>
  );
}

/* ── horizontal step indicator: mono numbered nodes on a rail ────────────── */
const STEPS = ["Detect", "Command", "Mode", "Recovery", "Review"] as const;
type StepName = (typeof STEPS)[number];

function StepRail({ active }: { active: number }) {
  return (
    <ol className="mb-7 flex items-center gap-0 select-none">
      {STEPS.map((label, i) => {
        const done = i < active;
        const current = i === active;
        return (
          <li key={label} className="flex flex-1 items-center last:flex-none">
            <div className="flex items-center gap-2.5">
              <span
                className={
                  "flex h-7 w-7 shrink-0 items-center justify-center rounded-md font-mono text-xs font-semibold ring-1 ring-inset transition " +
                  (current
                    ? "bg-signal-500 text-white ring-signal-400 shadow-glow"
                    : done
                      ? "bg-signal-500/15 text-signal-600 ring-signal-500/30 dark:text-signal-400"
                      : "bg-white/60 text-stone-400 ring-stone-300 dark:bg-white/[0.03] dark:text-stone-500 dark:ring-white/10")
                }
              >
                {done ? "✓" : i + 1}
              </span>
              <span
                className={
                  "hidden text-[11px] font-medium uppercase tracking-[0.12em] sm:block " +
                  (current
                    ? "text-stone-900 dark:text-stone-50"
                    : "text-stone-400 dark:text-stone-500")
                }
              >
                {label}
              </span>
            </div>
            {i < STEPS.length - 1 && (
              <span
                aria-hidden
                className={
                  "mx-3 h-px flex-1 " +
                  (done
                    ? "bg-signal-500/40"
                    : "bg-stone-200 dark:bg-white/[0.07]")
                }
              />
            )}
          </li>
        );
      })}
    </ol>
  );
}

/* ── tab switch used by mode selector ─────────────────────────────────────── */
function ModeTabs({
  value,
  onChange,
}: {
  value: RunModeAny["type"];
  onChange: (t: RunModeAny["type"]) => void;
}) {
  const tabs: RunModeAny["type"][] = ["daemon", "scheduled", "batch"];
  return (
    <div className="inline-flex rounded-lg border border-stone-200/80 bg-white/60 p-1 dark:border-white/[0.07] dark:bg-white/[0.03]">
      {tabs.map((t) => (
        <button
          key={t}
          type="button"
          onClick={() => onChange(t)}
          className={
            "rounded-md px-3.5 py-1.5 font-mono text-xs font-semibold uppercase tracking-wide transition " +
            (value === t
              ? "bg-signal-500 text-white shadow-sm"
              : "text-stone-500 hover:text-stone-800 dark:text-stone-400 dark:hover:text-stone-200")
          }
        >
          {t}
        </button>
      ))}
    </div>
  );
}

/* ─────────────────────────────────────────────────────────────────────────── */
export function CreateFlow({
  onDone,
  onCancel,
}: {
  onDone: () => void;
  onCancel: () => void;
}) {
  const [step, setStep] = useState<StepName>("Detect");
  const activeIdx = STEPS.indexOf(step);

  /* detect state */
  const [folder, setFolder] = useState("");
  const [scanning, setScanning] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanned, setScanned] = useState(false);
  const [suggestions, setSuggestions] = useState<SuggestionDraft[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  /* command step */
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [dir, setDir] = useState(".");

  /* mode step */
  const [mode, setMode] = useState<RunModeAny["type"]>("daemon");
  const [concurrency, setConcurrency] = useState(1);
  const [cronMode, setCronMode] = useState(true);
  const [cron, setCron] = useState("*/5 * * * *");
  const [intervalSecs, setIntervalSecs] = useState(60);
  const [runCount, setRunCount] = useState(1);
  const [delaySecs, setDelaySecs] = useState(0);

  /* recovery step */
  const [restartKind, setRestartKind] = useState("on_failure");
  const [maxRetries, setMaxRetries] = useState(5);
  const [autostart, setAutostart] = useState(true);

  const [saving, setSaving] = useState(false);

  async function scan() {
    setScanning(true);
    setScanError(null);
    try {
      const res = await api.detectWorkers(folder || ".");
      setSuggestions(res);
      setSelected(res.length ? new Set([0]) : new Set());
      setScanned(true);
    } catch (err) {
      setScanError(String(err));
      setSuggestions([]);
      setScanned(true);
    } finally {
      setScanning(false);
    }
  }

  function toggle(i: number) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      return next;
    });
  }

  /** Seed the wizard fields from a draft and advance to Command. */
  function adopt(draft: SuggestionDraft) {
    setName(draft.name);
    setCommand(draft.command);
    setArgs(draft.args.join(" "));
    setDir(draft.working_dir || ".");
    const rm = draft.run_mode;
    setMode(rm.type);
    if (rm.type === "daemon") setConcurrency(rm.concurrency);
    else if (rm.type === "scheduled") {
      if ("cron" in rm.schedule) {
        setCronMode(true);
        setCron(rm.schedule.cron);
      } else {
        setCronMode(false);
        setIntervalSecs(rm.schedule.interval_secs);
      }
    } else if (rm.type === "batch") {
      setRunCount(rm.run_count);
      setDelaySecs(rm.delay_secs);
    }
    setStep("Command");
  }

  function continueFromDetect() {
    const firstIdx = [...selected].sort((a, b) => a - b)[0];
    const draft = firstIdx != null ? suggestions[firstIdx] : undefined;
    if (draft) adopt(draft);
  }

  function fromScratch() {
    setName("");
    setCommand("");
    setArgs("");
    setDir(folder || ".");
    setMode("daemon");
    setStep("Command");
  }

  function buildRunMode(): RunModeAny {
    if (mode === "daemon") return { type: "daemon", concurrency };
    if (mode === "scheduled")
      return {
        type: "scheduled",
        schedule: cronMode ? { cron } : { interval_secs: intervalSecs },
        overlap: "skip",
      };
    return { type: "batch", run_count: runCount, delay_secs: delaySecs };
  }

  function buildSpec(): AddWorkerSpec {
    return {
      name,
      command,
      args: args.trim() ? args.trim().split(/\s+/) : [],
      working_dir: dir || ".",
      env: {},
      // run_mode is typed narrowly on AddWorkerSpec; the daemon shape is a
      // subset, so we widen here to carry scheduled/batch to the daemon.
      run_mode: buildRunMode() as unknown as AddWorkerSpec["run_mode"],
      restart: {
        kind: restartKind,
        max_retries: maxRetries,
        base_secs: 1,
        max_secs: 60,
        reset_window_secs: 30,
      },
      autostart,
      enabled: true,
    };
  }

  async function confirm() {
    if (!command) return;
    setSaving(true);
    try {
      await api.addWorker(buildSpec());
      onDone();
    } catch (err) {
      setScanError(String(err));
      setStep("Review");
    } finally {
      setSaving(false);
    }
  }

  const goto = (s: StepName) => setStep(s);
  const commandValid = command.trim().length > 0;

  return (
    <div className="mx-auto max-w-2xl p-6">
      {/* heading */}
      <div className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h2 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
            New worker
          </h2>
          <p className="mt-1 text-sm text-stone-500 dark:text-stone-400">
            Detect candidates in a folder, then tune supervision.
          </p>
        </div>
        <button className="btn-ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>

      <StepRail active={activeIdx} />

      {/* ── DETECT ─────────────────────────────────────────────────────────── */}
      {step === "Detect" && (
        <div className="animate-riseIn space-y-5">
          <div className={`${panelCls} p-6`}>
            <span className="absolute inset-y-0 left-0 w-1 bg-signal-500/40" aria-hidden />
            <legend className="mb-3 font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
              Detect from folder
            </legend>
            <div className="flex items-end gap-3">
              <div className="flex-1">
                <Field id="cf-folder" label="Folder" hint="path to a project root">
                  <input
                    id="cf-folder"
                    className={fieldCls}
                    placeholder="/srv/app"
                    value={folder}
                    onChange={(e) => setFolder(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && scan()}
                  />
                </Field>
              </div>
              <button className="btn-primary" onClick={scan} disabled={scanning}>
                {scanning ? "Scanning…" : "Scan"}
              </button>
            </div>

            {scanError && (
              <div className="mt-4 flex items-start gap-2 rounded-md border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-xs text-rose-700 dark:text-rose-300">
                <span className="font-mono font-bold">!</span>
                <span className="break-words">Detection failed: {scanError}</span>
              </div>
            )}
          </div>

          {/* suggestion rows */}
          {scanned && !scanError && (
            <div className="space-y-2.5">
              <div className="flex items-center justify-between px-1">
                <span className="font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
                  {suggestions.length} suggestion{suggestions.length === 1 ? "" : "s"}
                </span>
              </div>

              {suggestions.length === 0 && (
                <p className="rounded-lg border border-dashed border-stone-300 bg-white/40 px-4 py-6 text-center text-sm text-stone-500 dark:border-white/10 dark:bg-white/[0.02] dark:text-stone-400">
                  No workers detected here — start from scratch below.
                </p>
              )}

              {suggestions.map((d, i) => {
                const on = selected.has(i);
                return (
                  <label
                    key={i}
                    className={
                      "group flex cursor-pointer items-center gap-3 rounded-xl border px-4 py-3 transition " +
                      (on
                        ? "border-signal-400/60 bg-signal-500/[0.06] ring-1 ring-inset ring-signal-400/30 dark:border-signal-400/40"
                        : "border-stone-200/80 bg-white hover:border-stone-300 dark:border-white/[0.06] dark:bg-[#13161b] dark:hover:border-white/15")
                    }
                  >
                    <input
                      type="checkbox"
                      className="h-4 w-4 rounded border-stone-300 text-signal-500 accent-signal-500 focus:ring-signal-400 dark:border-white/20"
                      checked={on}
                      onChange={() => toggle(i)}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="truncate font-display text-sm font-semibold text-stone-900 dark:text-stone-50">
                          {d.label}
                        </span>
                        <span className="font-mono text-[10px] uppercase tracking-wide text-stone-400 dark:text-stone-500">
                          {d.source}
                        </span>
                      </div>
                      <div className="mt-0.5 truncate font-mono text-[11px] text-stone-500 dark:text-stone-400">
                        {[d.command, ...d.args].filter(Boolean).join(" ") || "—"}
                      </div>
                    </div>
                    <ModeChip type={d.run_mode.type} />
                  </label>
                );
              })}

              <div className="flex items-center justify-between gap-3 pt-1">
                <button
                  className="font-mono text-xs text-stone-500 underline-offset-4 transition hover:text-signal-600 hover:underline dark:hover:text-signal-400"
                  onClick={fromScratch}
                >
                  + Start from scratch
                </button>
                <button
                  className="btn-primary"
                  onClick={continueFromDetect}
                  disabled={selected.size === 0}
                >
                  Continue →
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ── COMMAND ────────────────────────────────────────────────────────── */}
      {step === "Command" && (
        <div className="animate-riseIn space-y-6">
          <div className={`${panelCls} space-y-4 p-6`}>
            <span className="absolute inset-y-0 left-0 w-1 bg-signal-500/40" aria-hidden />
            <legend className="font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
              Command
            </legend>
            <Field id="cf-name" label="Name">
              <input id="cf-name" className={fieldCls} placeholder="queue-worker" value={name} onChange={(e) => setName(e.target.value)} />
            </Field>
            <Field id="cf-cmd" label="Command">
              <input id="cf-cmd" className={fieldCls} placeholder="php" value={command} onChange={(e) => setCommand(e.target.value)} />
            </Field>
            <Field id="cf-args" label="Args" hint="space-separated">
              <input id="cf-args" className={fieldCls} placeholder="artisan queue:work" value={args} onChange={(e) => setArgs(e.target.value)} />
            </Field>
            <Field id="cf-dir" label="Working dir">
              <input id="cf-dir" className={fieldCls} placeholder="." value={dir} onChange={(e) => setDir(e.target.value)} />
            </Field>
          </div>
          <StepNav onBack={() => goto("Detect")} onNext={() => goto("Mode")} nextDisabled={!commandValid} />
        </div>
      )}

      {/* ── MODE ───────────────────────────────────────────────────────────── */}
      {step === "Mode" && (
        <div className="animate-riseIn space-y-6">
          <div className={`${panelCls} space-y-5 p-6`}>
            <span className="absolute inset-y-0 left-0 w-1 bg-signal-500/40" aria-hidden />
            <div className="flex items-center justify-between">
              <legend className="font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
                Run mode
              </legend>
              <ModeChip type={mode} />
            </div>
            <ModeTabs value={mode} onChange={setMode} />

            {mode === "daemon" && (
              <Field id="cf-conc" label="Concurrency" hint="instances kept alive">
                <input id="cf-conc" type="number" min={1} className={fieldCls} value={concurrency} onChange={(e) => setConcurrency(+e.target.value)} />
              </Field>
            )}

            {mode === "scheduled" && (
              <div className="space-y-4">
                <div className="inline-flex rounded-lg border border-stone-200/80 bg-white/60 p-1 dark:border-white/[0.07] dark:bg-white/[0.03]">
                  {[
                    { k: true, l: "cron" },
                    { k: false, l: "interval" },
                  ].map(({ k, l }) => (
                    <button
                      key={l}
                      type="button"
                      onClick={() => setCronMode(k)}
                      className={
                        "rounded-md px-3 py-1 font-mono text-xs font-semibold transition " +
                        (cronMode === k
                          ? "bg-signal-500 text-white"
                          : "text-stone-500 hover:text-stone-800 dark:text-stone-400 dark:hover:text-stone-200")
                      }
                    >
                      {l}
                    </button>
                  ))}
                </div>
                {cronMode ? (
                  <Field id="cf-cron" label="Cron expression">
                    <input id="cf-cron" className={fieldCls} placeholder="*/5 * * * *" value={cron} onChange={(e) => setCron(e.target.value)} />
                  </Field>
                ) : (
                  <Field id="cf-interval" label="Interval" hint="seconds">
                    <input id="cf-interval" type="number" min={1} className={fieldCls} value={intervalSecs} onChange={(e) => setIntervalSecs(+e.target.value)} />
                  </Field>
                )}
                <p className="font-mono text-[11px] text-stone-400 dark:text-stone-500">
                  overlap: <span className="text-stone-600 dark:text-stone-300">skip</span> — a run waits for the previous to finish.
                </p>
              </div>
            )}

            {mode === "batch" && (
              <div className="grid grid-cols-2 gap-4">
                <Field id="cf-runs" label="Run count">
                  <input id="cf-runs" type="number" min={1} className={fieldCls} value={runCount} onChange={(e) => setRunCount(+e.target.value)} />
                </Field>
                <Field id="cf-delay" label="Delay" hint="seconds between runs">
                  <input id="cf-delay" type="number" min={0} className={fieldCls} value={delaySecs} onChange={(e) => setDelaySecs(+e.target.value)} />
                </Field>
              </div>
            )}
          </div>
          <StepNav onBack={() => goto("Command")} onNext={() => goto("Recovery")} />
        </div>
      )}

      {/* ── RECOVERY ───────────────────────────────────────────────────────── */}
      {step === "Recovery" && (
        <div className="animate-riseIn space-y-6">
          <div className={`${panelCls} space-y-5 p-6`}>
            <span className="absolute inset-y-0 left-0 w-1 bg-signal-500/40" aria-hidden />
            <legend className="font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
              Recovery
            </legend>
            <Field id="cf-restart" label="Restart policy">
              <div className="inline-flex rounded-lg border border-stone-200/80 bg-white/60 p-1 dark:border-white/[0.07] dark:bg-white/[0.03]">
                {["always", "on_failure", "never"].map((k) => (
                  <button
                    key={k}
                    type="button"
                    onClick={() => setRestartKind(k)}
                    className={
                      "rounded-md px-3 py-1.5 font-mono text-xs font-semibold transition " +
                      (restartKind === k
                        ? "bg-signal-500 text-white"
                        : "text-stone-500 hover:text-stone-800 dark:text-stone-400 dark:hover:text-stone-200")
                    }
                  >
                    {k}
                  </button>
                ))}
              </div>
            </Field>
            <Field id="cf-retries" label="Max retries">
              <input id="cf-retries" type="number" min={0} className={fieldCls} value={maxRetries} onChange={(e) => setMaxRetries(+e.target.value)} />
            </Field>
            <label className="flex cursor-pointer items-center gap-2.5 text-sm text-stone-600 dark:text-stone-300">
              <input
                type="checkbox"
                className="h-4 w-4 rounded border-stone-300 text-signal-500 accent-signal-500 focus:ring-signal-400 dark:border-white/20"
                checked={autostart}
                onChange={(e) => setAutostart(e.target.checked)}
              />
              Autostart on daemon boot
            </label>
          </div>
          <StepNav onBack={() => goto("Mode")} onNext={() => goto("Review")} />
        </div>
      )}

      {/* ── REVIEW ─────────────────────────────────────────────────────────── */}
      {step === "Review" && (
        <div className="animate-riseIn space-y-6">
          <div className={`${panelCls} p-6`}>
            <span className="absolute inset-y-0 left-0 w-1 bg-signal-500/40" aria-hidden />
            <div className="mb-3 flex items-center justify-between">
              <legend className="font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
                Review spec
              </legend>
              <ModeChip type={mode} />
            </div>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
              {[
                ["name", name || "—"],
                ["command", command || "—"],
                ["args", args.trim() || "—"],
                ["working_dir", dir || "."],
                ["restart", `${restartKind} · ≤${maxRetries}`],
                ["autostart", autostart ? "yes" : "no"],
              ].map(([k, v]) => (
                <div key={k}>
                  <dt className="text-[10px] uppercase tracking-[0.14em] text-stone-400 dark:text-stone-500">{k}</dt>
                  <dd className="mt-0.5 truncate font-mono text-stone-800 dark:text-stone-100">{v}</dd>
                </div>
              ))}
            </dl>
            <pre className="scroll-thin mt-5 max-h-56 overflow-auto rounded-lg border border-white/10 bg-[#0a0c10] p-4 font-mono text-[11px] leading-relaxed text-stone-300">
              {JSON.stringify(buildSpec(), null, 2)}
            </pre>
            {scanError && (
              <div className="mt-3 flex items-start gap-2 rounded-md border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-xs text-rose-700 dark:text-rose-300">
                <span className="font-mono font-bold">!</span>
                <span className="break-words">{scanError}</span>
              </div>
            )}
          </div>
          <div className="flex items-center justify-between">
            <button className="btn-ghost" onClick={() => goto("Recovery")}>
              ← Back
            </button>
            <button className="btn-primary" onClick={confirm} disabled={saving || !commandValid}>
              {saving ? "Creating…" : "Create worker"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function StepNav({
  onBack,
  onNext,
  nextDisabled,
}: {
  onBack: () => void;
  onNext: () => void;
  nextDisabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between">
      <button className="btn-ghost" onClick={onBack}>
        ← Back
      </button>
      <button className="btn-primary" onClick={onNext} disabled={nextDisabled}>
        Next →
      </button>
    </div>
  );
}
