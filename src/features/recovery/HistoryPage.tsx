import { useEffect, useState } from "react";
import type { PortEntry, ProcessAction } from "../../lib/types";
import { TimelinePage } from "../timeline/TimelinePage";
import { RunHistoryPage } from "./RunHistoryPage";

export function HistoryPage({
  initialPort,
  ports,
  favorites,
  paused,
  failed,
  inspect,
  requestAction,
  toggleFavorite,
  copy,
  refreshPorts,
  notify,
}: {
  initialPort?: number;
  ports: PortEntry[];
  favorites: number[];
  paused: boolean;
  failed: boolean;
  inspect: (entry: PortEntry) => void;
  requestAction: (entry: PortEntry, action: ProcessAction) => void;
  toggleFavorite: (port: number) => void;
  copy: (value: string, label: string) => void;
  refreshPorts: () => Promise<unknown>;
  notify: (message: string, error?: boolean) => void;
}) {
  const [view, setView] = useState<"runs" | "timeline">(
    initialPort ? "timeline" : "runs",
  );
  const [port, setPort] = useState<number | undefined>(initialPort);
  useEffect(() => {
    if (initialPort) {
      setPort(initialPort);
      setView("timeline");
    }
  }, [initialPort]);
  return (
    <div className="history-workspace">
      <div
        className="history-mode-tabs"
        role="tablist"
        aria-label="History mode"
      >
        <button
          role="tab"
          aria-selected={view === "runs"}
          onClick={() => setView("runs")}
        >
          Run History
        </button>
        <button
          role="tab"
          aria-selected={view === "timeline"}
          onClick={() => setView("timeline")}
        >
          Port Timeline
        </button>
      </div>
      {view === "runs" ? (
        <RunHistoryPage
          ports={ports}
          favorites={favorites}
          inspect={inspect}
          requestAction={requestAction}
          openTimeline={(selectedPort) => {
            setPort(selectedPort);
            setView("timeline");
          }}
          copy={copy}
          refreshPorts={refreshPorts}
          notify={notify}
        />
      ) : (
        <TimelinePage
          port={port}
          selectPort={setPort}
          ports={ports}
          favorites={favorites}
          paused={paused}
          failed={failed}
          inspect={inspect}
          requestAction={requestAction}
          toggleFavorite={toggleFavorite}
          refreshPorts={refreshPorts}
          notify={notify}
        />
      )}
    </div>
  );
}
