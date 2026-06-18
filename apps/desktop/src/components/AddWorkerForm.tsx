import { useState } from "react";
import type { AddWorkerSpec } from "../api";

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

export function AddWorkerForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: (spec: AddWorkerSpec) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [dir, setDir] = useState("");
  const [concurrency, setConcurrency] = useState(1);
  const [maxRetries, setMaxRetries] = useState(5);
  const [autostart, setAutostart] = useState(true);

  function submit() {
    if (!name || !command) return;
    onSubmit({
      name,
      command,
      args: args.trim() ? args.trim().split(/\s+/) : [],
      working_dir: dir || ".",
      env: {},
      run_mode: { type: "daemon", concurrency },
      restart: {
        kind: "on_failure",
        max_retries: maxRetries,
        base_secs: 1,
        max_secs: 60,
        reset_window_secs: 30,
      },
      autostart,
      enabled: true,
    });
  }

  const field =
    "w-full rounded-md border border-stone-300 bg-white px-3 py-2 font-mono text-sm text-stone-900 " +
    "shadow-sm transition placeholder:text-stone-400 focus:border-signal-400 focus:outline-none " +
    "focus:ring-2 focus:ring-signal-400/30 dark:border-white/10 dark:bg-white/[0.03] dark:text-stone-100 " +
    "dark:placeholder:text-stone-600";

  return (
    <div className="mx-auto max-w-2xl p-6">
      <div className="mb-6">
        <h2 className="font-display text-2xl font-bold tracking-tight text-stone-900 dark:text-stone-50">
          New worker
        </h2>
        <p className="mt-1 text-sm text-stone-500 dark:text-stone-400">
          Define a process for servicio to supervise.
        </p>
      </div>

      <div className="space-y-6 rounded-xl border border-stone-200/80 bg-white p-6 shadow-panel
        dark:border-white/[0.06] dark:bg-[#13161b] dark:shadow-panel-dark">
        {/* identity group */}
        <fieldset className="space-y-4">
          <legend className="mb-2 font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
            Command
          </legend>
          <Field id="w-name" label="Name">
            <input
              id="w-name"
              className={field}
              placeholder="queue-worker"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </Field>
          <Field id="w-cmd" label="Command">
            <input
              id="w-cmd"
              className={field}
              placeholder="php"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
            />
          </Field>
          <Field id="w-args" label="Args" hint="space-separated">
            <input
              id="w-args"
              className={field}
              placeholder="artisan queue:work"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
            />
          </Field>
          <Field id="w-dir" label="Working dir">
            <input
              id="w-dir"
              className={field}
              placeholder="."
              value={dir}
              onChange={(e) => setDir(e.target.value)}
            />
          </Field>
        </fieldset>

        {/* supervision group */}
        <fieldset className="space-y-4 border-t border-stone-100 pt-5 dark:border-white/[0.05]">
          <legend className="mb-2 font-mono text-[11px] uppercase tracking-[0.16em] text-stone-400 dark:text-stone-500">
            Supervision
          </legend>
          <div className="grid grid-cols-2 gap-4">
            <Field id="w-conc" label="Concurrency">
              <input
                id="w-conc"
                type="number"
                min={1}
                className={field}
                value={concurrency}
                onChange={(e) => setConcurrency(+e.target.value)}
              />
            </Field>
            <Field id="w-retries" label="Max retries">
              <input
                id="w-retries"
                type="number"
                min={0}
                className={field}
                value={maxRetries}
                onChange={(e) => setMaxRetries(+e.target.value)}
              />
            </Field>
          </div>
          <label className="flex cursor-pointer items-center gap-2.5 text-sm text-stone-600 dark:text-stone-300">
            <input
              type="checkbox"
              className="h-4 w-4 rounded border-stone-300 text-signal-500 accent-signal-500 focus:ring-signal-400 dark:border-white/20"
              checked={autostart}
              onChange={(e) => setAutostart(e.target.checked)}
            />
            Autostart on daemon boot
          </label>
        </fieldset>
      </div>

      <div className="mt-6 flex gap-3">
        <button className="btn-primary" onClick={submit}>
          Create worker
        </button>
        <button className="btn-ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
