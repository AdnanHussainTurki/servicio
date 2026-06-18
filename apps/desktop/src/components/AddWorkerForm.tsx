import { useState } from "react";
import type { AddWorkerSpec } from "../api";

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
      restart: { kind: "on_failure", max_retries: maxRetries, base_secs: 1, max_secs: 60, reset_window_secs: 30 },
      autostart,
      enabled: true,
    });
  }

  const field =
    "w-full rounded border border-slate-300 dark:border-slate-700 bg-transparent px-2 py-1 text-sm";
  return (
    <div className="p-6 max-w-lg">
      <h2 className="text-xl font-semibold mb-4">New worker</h2>
      <div className="mb-2">
        <label htmlFor="w-name" className="block text-xs mb-1">Name</label>
        <input id="w-name" className={field} value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div className="mb-2">
        <label htmlFor="w-cmd" className="block text-xs mb-1">Command</label>
        <input id="w-cmd" className={field} value={command} onChange={(e) => setCommand(e.target.value)} />
      </div>
      <div className="mb-2">
        <label htmlFor="w-args" className="block text-xs mb-1">Args (space-separated)</label>
        <input id="w-args" className={field} value={args} onChange={(e) => setArgs(e.target.value)} />
      </div>
      <div className="mb-2">
        <label htmlFor="w-dir" className="block text-xs mb-1">Working dir</label>
        <input id="w-dir" className={field} value={dir} onChange={(e) => setDir(e.target.value)} />
      </div>
      <div className="mb-2">
        <label htmlFor="w-conc" className="block text-xs mb-1">Concurrency</label>
        <input id="w-conc" type="number" min={1} className={field} value={concurrency} onChange={(e) => setConcurrency(+e.target.value)} />
      </div>
      <div className="mb-2">
        <label htmlFor="w-retries" className="block text-xs mb-1">Max retries</label>
        <input id="w-retries" type="number" min={0} className={field} value={maxRetries} onChange={(e) => setMaxRetries(+e.target.value)} />
      </div>
      <label className="flex items-center gap-2 text-xs mb-4">
        <input type="checkbox" checked={autostart} onChange={(e) => setAutostart(e.target.checked)} /> Autostart on daemon boot
      </label>
      <div className="flex gap-2">
        <button className="rounded bg-blue-600 text-white text-sm px-3 py-1.5" onClick={submit}>Create</button>
        <button className="rounded bg-slate-200 dark:bg-slate-700 text-sm px-3 py-1.5" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}
