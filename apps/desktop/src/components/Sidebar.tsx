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
  const ok = daemon?.connected;
  const { theme, toggle } = useTheme();

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
            servicio
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

      {/* daemon status pill */}
      <div className="mx-3 mb-3 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${
              ok ? "animate-pulseDot bg-emerald-500" : "bg-rose-500"
            }`}
            style={{ "--dot": "rgba(16,185,129,0.55)" } as React.CSSProperties}
          />
          <span className="font-mono text-[11px] font-medium text-stone-300">
            {ok ? "daemon online" : "disconnected"}
          </span>
        </div>
        {ok && (
          <div className="mt-1 pl-4 font-mono text-[10px] text-stone-500">
            v{daemon?.version} · {daemon?.running_count} running
          </div>
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
    </nav>
  );
}
