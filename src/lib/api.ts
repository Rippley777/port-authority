import {
  recordPreviewRelease,
  queryTimeline,
  configureTimeline,
} from "../features/timeline/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { demoPorts } from "./demo";
import type { PortEntry, ProcessAction } from "./types";
export const desktop = isTauri();
let previewPorts = [...demoPorts];
let historyMigration: Promise<boolean> | undefined;
async function migrateHistoryPreference(): Promise<boolean> {
  try {
    if (localStorage.getItem("pa-timeline-migrated")) return true;
    const old = JSON.parse(localStorage.getItem("pa-settings") ?? "null");
    if (old?.keepHistory === false) {
      const page = await queryTimeline({ limit: 1 });
      await configureTimeline({ ...page.config, enabled: false });
    }
    localStorage.setItem("pa-timeline-migrated", "1");
    return true;
  } catch {
    // If migration fails, keep inspection usable without accidentally enabling recording.
    historyMigration = undefined;
    return false;
  }
}
export async function scanPorts(): Promise<PortEntry[]> {
  if (!desktop) return [...previewPorts];
  historyMigration ??= migrateHistoryPreference();
  return invoke("scan_ports", { recordHistory: await historyMigration });
}
export async function controlProcess(
  entry: PortEntry,
  action: ProcessAction,
): Promise<string | void> {
  if (entry.protected || !entry.pid)
    throw new Error("This process is protected. Process control is disabled.");
  if ((action === "restart" || action === "relaunch") && !entry.restartable)
    throw new Error(entry.restartReason);
  if (action === "restart" || action === "relaunch") {
    if (!desktop)
      throw new Error(
        "Command Recovery requires the desktop app. No process was affected.",
      );
    if (!entry.launch || !entry.startedAt)
      throw new Error("Original launch command could not be determined.");
    const result = await invoke<
      import("../features/recovery/types").RecoveryStatus
    >("recovery_restart", {
      id: entry.launch.id,
      identity: { pid: entry.pid, startedAt: entry.startedAt },
      port: entry.port,
      force: action === "relaunch",
    });
    return result.message;
  }
  if (desktop)
    await invoke("control_process", {
      pid: entry.pid,
      startedAt: entry.startedAt,
      action,
    });
  else {
    for (const p of previewPorts.filter((p) => p.pid === entry.pid))
      recordPreviewRelease(p);
    previewPorts = previewPorts.filter((p) => p.pid !== entry.pid);
  }
}
export async function openPort(entry: PortEntry, protocol: "http" | "https") {
  if (desktop) await invoke("open_port", { port: entry.port, protocol });
  else
    window.open(
      `${protocol}://localhost:${entry.port}`,
      "_blank",
      "noopener,noreferrer",
    );
}
export async function revealExecutable(entry: PortEntry) {
  if (!desktop)
    throw new Error(
      "Reveal executable is available in the desktop application.",
    );
  await invoke("reveal_executable", {
    pid: entry.pid,
    startedAt: entry.startedAt,
  });
}

export async function writeClipboard(text: string): Promise<void> {
  if (desktop) await writeText(text);
  else await navigator.clipboard.writeText(text);
}
