export function Sidebar({ view, setView }: { view: string; setView: (v: string) => void }) {
  const item = (id: string, label: string) =>
    <button onClick={() => setView(id)}
      className={`text-left px-3 py-2 rounded ${view === id ? "bg-slate-200 dark:bg-slate-800" : "opacity-70"}`}>{label}</button>;
  return (
    <nav className="w-44 shrink-0 p-3 flex flex-col gap-1 border-r border-slate-200 dark:border-slate-800">
      <div className="font-bold mb-3 px-3">⚙ servicio</div>
      {item("dashboard", "▦ Dashboard")}
      {item("settings", "⚙ Settings")}
    </nav>
  );
}
