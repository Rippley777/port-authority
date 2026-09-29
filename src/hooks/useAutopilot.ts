import { useCallback, useEffect, useRef, useState } from "react";
import { desktop } from "../lib/api";
import {
  actOnConflict,
  enableAutopilot,
  getAutopilot,
  simulateConflict,
} from "../lib/autopilot";
import type {
  AutopilotSnapshot,
  ConflictAction,
  PreviewScenario,
} from "../lib/autopilot";
export function useAutopilot(refreshPorts: () => Promise<void>) {
  const [snapshot, setSnapshot] = useState<AutopilotSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  const refresh = useCallback(async () => {
    try {
      setSnapshot(await getAutopilot());
    } catch (e) {
      setError(String(e));
    }
  }, []);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const next = await getAutopilot();
        if (!cancelled) setSnapshot(next);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
      if (!cancelled)
        timer = setTimeout(() => {
          void poll();
        }, 1000);
    }
    async function start() {
      // Enabling is explicit and remembered only on this device. Browser preview
      // preferences never automatically enable native shell capture.
      if (desktop) {
        try {
          if (localStorage.getItem("pa-autopilot-opt-in") === "true")
            await enableAutopilot(true);
        } catch (e) {
          if (!cancelled) setError(String(e));
        }
      }
      if (!cancelled) await poll();
    }
    void start();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);
  const run = useCallback(
    async (
      id: string,
      action: ConflictAction,
      approved = false,
      targetPort: number | null = null,
    ) => {
      if (locked.current) return;
      locked.current = true;
      setBusy(true);
      setError(null);
      try {
        await actOnConflict(id, action, approved, targetPort);
        await refreshPorts();
      } catch (e) {
        setError(String(e));
      } finally {
        await refresh();
        setBusy(false);
        locked.current = false;
      }
    },
    [refresh, refreshPorts],
  );
  const toggle = useCallback(
    async (enabled: boolean) => {
      if (locked.current) return;
      locked.current = true;
      setBusy(true);
      setError(null);
      try {
        await enableAutopilot(enabled);
        if (desktop) {
          try {
            localStorage.setItem("pa-autopilot-opt-in", String(enabled));
          } catch {
            /* Keep this session’s explicit choice. */
          }
        }
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        locked.current = false;
        setBusy(false);
      }
    },
    [refresh],
  );
  const simulate = useCallback(
    async (scenario: PreviewScenario) => {
      setError(null);
      try {
        await simulateConflict(scenario);
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );
  return {
    snapshot,
    error,
    busy,
    run,
    toggle,
    simulate,
    clearError: () => setError(null),
  };
}
export type AutopilotController = ReturnType<typeof useAutopilot>;
