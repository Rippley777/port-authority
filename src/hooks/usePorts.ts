import { parseHistory } from "../lib/persistence";
import { useCallback, useEffect, useRef, useState } from "react";
import { scanPorts, desktop } from "../lib/api";
import { demoPorts } from "../lib/demo";
import { reconcile } from "../lib/ports";
import type { HistoryEvent, PortEntry, Settings } from "../lib/types";
import { usePersistentState } from "./usePersistentState";
export function usePorts(settings: Settings) {
  const [ports, setPorts] = useState<PortEntry[]>(desktop ? [] : demoPorts);
  const [history, setHistory] = usePersistentState<HistoryEvent[]>(
    desktop ? "pa-history" : "pa-preview-history",
    [],
    parseHistory,
  );
  const [paused, setPaused] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastScan, setLastScan] = useState<Date | null>(null);
  const previous = useRef<PortEntry[] | null>(null);
  const busy = useRef(false);
  const refresh = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setScanning(true);
    try {
      const next = await scanPorts();
      if (previous.current && settings.keepHistory) {
        const before = new Map(previous.current.map((p) => [p.id, p]));
        const after = new Map(next.map((p) => [p.id, p]));
        const events: HistoryEvent[] = [];
        for (const [id, p] of after)
          if (!before.has(id))
            events.push({
              id: `${Date.now()}-start-${id}`,
              time: Date.now(),
              port: p.port,
              process: p.process,
              type: "started",
            });
        for (const [id, p] of before)
          if (!after.has(id))
            events.push({
              id: `${Date.now()}-stop-${id}`,
              time: Date.now(),
              port: p.port,
              process: p.process,
              type: "stopped",
            });
        if (events.length)
          setHistory((h) => [...events, ...h].slice(0, settings.retention));
      }
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
  }, [settings.keepHistory, settings.retention, setHistory]);
  useEffect(() => {
    if (paused) return;
    void refresh();
    const timer = setInterval(() => {
      void refresh();
    }, settings.refreshInterval * 1000);
    return () => clearInterval(timer);
  }, [paused, refresh, settings.refreshInterval]);
  useEffect(() => {
    setHistory((h) =>
      settings.keepHistory ? h.slice(0, settings.retention) : [],
    );
  }, [settings.keepHistory, settings.retention, setHistory]);
  return {
    ports,
    history,
    clearHistory: () => setHistory([]),
    paused,
    setPaused,
    scanning,
    error,
    lastScan,
    refresh,
  };
}
