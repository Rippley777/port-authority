import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { scanPorts, desktop } from "../lib/api";
import { demoPorts } from "../lib/demo";
import { reconcile } from "../lib/ports";
import type { PortEntry, Settings } from "../lib/types";
export function usePorts(settings: Settings) {
  const [ports, setPorts] = useState<PortEntry[]>(desktop ? [] : demoPorts);
  const [paused, setPaused] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastScan, setLastScan] = useState<Date | null>(null);
  const previous = useRef<PortEntry[] | null>(null);
  const busy = useRef(false);
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const subscription = listen<PortEntry[]>(
      "projects-resolved",
      ({ payload }) => {
        if (disposed) return;
        const updates = new Map(payload.map((p) => [p.id, p]));
        const enrich = (current: PortEntry[]) =>
          current.map((p) => {
            const next = updates.get(p.id);
            if (
              !next ||
              p.startedAt !== next.startedAt ||
              p.cwd !== next.cwd ||
              p.executable !== next.executable ||
              JSON.stringify(p.command) !== JSON.stringify(next.command)
            )
              return p;
            return {
              ...p,
              project: next.project,
              serviceName: next.serviceName,
            };
          });
        if (previous.current) previous.current = enrich(previous.current);
        setPorts((old) => reconcile(old, enrich(old)));
      },
    ).catch(() => undefined);
    return () => {
      disposed = true;
      void subscription.then((unlisten) => unlisten?.());
    };
  }, []);
  const refresh = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setScanning(true);
    try {
      const next = await scanPorts();
      previous.current = next;
      setPorts((old) => reconcile(old, next));
      setLastScan(new Date());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      busy.current = false;
      setScanning(false);
    }
  }, []);
  useEffect(() => {
    let disposed = false;
    let ready = false;
    const sync = desktop
      ? invoke("timeline_monitoring", { enabled: !paused })
      : Promise.resolve();
    void sync
      .catch(() => {})
      .then(() => {
        if (disposed || paused) return;
        ready = true;
        void refresh();
      });
    const timer = setInterval(() => {
      if (ready && !paused) void refresh();
    }, settings.refreshInterval * 1000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [paused, refresh, settings.refreshInterval]);
  return {
    ports,
    paused,
    setPaused,
    scanning,
    error,
    lastScan,
    refresh,
  };
}
