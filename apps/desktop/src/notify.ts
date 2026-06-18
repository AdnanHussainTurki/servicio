import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import type { StateEventPayload } from "./types";

let permission: "granted" | "denied" | "default" | "checking" = "checking";

async function ensurePermission(): Promise<boolean> {
  try {
    if (permission === "checking") {
      permission = (await isPermissionGranted()) ? "granted" : (await requestPermission()) as any;
    }
    return permission === "granted";
  } catch {
    return false;
  }
}

const DOWN = new Set(["crashed", "failed"]);
const RECOVER_FROM = new Set(["crashed", "backoff", "failed"]);

/** Fire a native notification for crash/crash-loop/recovery state transitions. */
export async function notifyStateEvent(e: StateEventPayload): Promise<void> {
  let title: string | null = null;
  let body: string | null = null;
  if (DOWN.has(e.to)) {
    title = e.to === "failed" ? `Worker "${e.worker}" failed` : `Worker "${e.worker}" crashed`;
    body = `Instance ${e.instance} → ${e.to}`;
  } else if (e.to === "running" && RECOVER_FROM.has(e.from)) {
    title = `Worker "${e.worker}" recovered`;
    body = `Instance ${e.instance} is running again`;
  }
  if (!title) return;
  try {
    if (await ensurePermission()) sendNotification({ title, body: body ?? "" });
  } catch {
    /* notifications unavailable (e.g. non-Tauri/dev browser) — ignore */
  }
}
