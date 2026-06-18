import type { CSSProperties } from "react";
import { SIGNAL_OF, SIGNAL_STYLE } from "./status";
import type { InstanceState } from "../types";

export function StatusDot({
  state,
  size = "md",
}: {
  state: InstanceState;
  size?: "sm" | "md";
}) {
  const sig = SIGNAL_OF[state];
  const s = SIGNAL_STYLE[sig];
  const dim = size === "sm" ? "h-2 w-2" : "h-2.5 w-2.5";
  return (
    <span
      className={`relative inline-flex shrink-0 rounded-full ${dim} ${s.dot} ${
        sig === "live" ? "animate-pulseDot" : ""
      }`}
      style={{ "--dot": s.dotVar } as CSSProperties}
      aria-hidden
    />
  );
}
