import { listen } from "@tauri-apps/api/event";
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
        setHistory((history) => {
          let changed = false;
          const next = history.map((event) => {
            if (
              !event.processPid ||
              !event.processStartedAt ||
              event.projectName
            )
              return event;
            const entry = payload.find(
              (p) =>
                p.pid === event.processPid &&
                p.startedAt === event.processStartedAt &&
                p.port === event.port &&
                p.project,
            );
            if (!entry?.project) return event;
            changed = true;
            return {
              ...event,
              projectName: entry.project.name,
              projectPath: entry.project.rootPath,
            };
          });
          return changed ? next : history;
        });
        if (previous.current) previous.current = enrich(previous.current);
        setPorts((old) => reconcile(old, enrich(old)));
      },
    ).catch(() => undefined);
    return () => {
      disposed = true;
      void subscription.then((unlisten) => unlisten?.());
    };
  }, [setHistory]);
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
              processPid: p.pid,
              processStartedAt: p.startedAt,
              projectName: p.project?.name,
              projectPath: p.project?.rootPath,
              type: "started",
            });
        for (const [id, p] of before)
          if (!after.has(id))
            events.push({
              id: `${Date.now()}-stop-${id}`,
              time: Date.now(),
              port: p.port,
              process: p.process,
              processPid: p.pid,
              processStartedAt: p.startedAt,
              projectName: p.project?.name,
              projectPath: p.project?.rootPath,
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
