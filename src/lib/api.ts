import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { demoPorts } from "./demo";
import type { PortEntry, ProcessAction } from "./types";
export const desktop = isTauri();
let previewPorts = [...demoPorts];
export async function scanPorts(): Promise<PortEntry[]> {
  return desktop ? invoke("scan_ports") : [...previewPorts];
}
export async function controlProcess(
  entry: PortEntry,
  action: ProcessAction,
): Promise<void> {
  if (entry.protected || !entry.pid)
    throw new Error("This process is protected. Process control is disabled.");
  if (action === "restart" && !entry.restartable)
    throw new Error(entry.restartReason);
  if (desktop)
    await invoke("control_process", {
      pid: entry.pid,
      startedAt: entry.startedAt,
      action,
    });
  else {
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
