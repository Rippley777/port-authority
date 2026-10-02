import { Clock3, ExternalLink, History, Pin, Play, Star } from "lucide-react";
import { useState } from "react";
import type { PortEntry, ProcessAction } from "../../lib/types";
import {
  historicalCommandNeedsConfirmation,
  pinHistoricalCommand,
  runHistoricalCommand,
} from "./api";
import { RunAgainDialog } from "./RunAgainDialog";
import type { HistoricalCommand } from "./types";
import { useRunHistory } from "./useRunHistory";

export function FavoritesPage({
  favorites,
  ports,
  paused,
  toggleFavorite,
  inspect,
  requestAction,
  openTimeline,
  refreshPorts,
  notify,
}: {
  favorites: number[];
  ports: PortEntry[];
  paused: boolean;
  toggleFavorite: (port: number) => void;
  inspect: (entry: PortEntry) => void;
  requestAction: (entry: PortEntry, action: ProcessAction) => void;
  openTimeline: (port: number) => void;
  refreshPorts: () => Promise<unknown>;
  notify: (message: string, error?: boolean) => void;
}) {
  const history = useRunHistory({ limit: 1000 });
  const [expanded, setExpanded] = useState<number>();
  const [busy, setBusy] = useState<string>();
  const [confirmation, setConfirmation] = useState<HistoricalCommand | null>(
    null,
  );

  async function run(command: HistoricalCommand, confirmed = false) {
    if (
      historicalCommandNeedsConfirmation(command.launchContext) &&
      !confirmed
    ) {
      setConfirmation(command);
      return;
    }
    setBusy(command.launchContext.id);
    try {
      const result = await runHistoricalCommand(
        command.launchContext.id,
        confirmed,
      );
      notify(result.message);
      await Promise.all([history.refresh(), refreshPorts()]);
    } catch (reason) {
      notify(String(reason), true);
    } finally {
      setBusy(undefined);
    }
  }

  async function pin(port: number, command: HistoricalCommand) {
    try {
      const pinned = !command.pinnedPorts.includes(port);
      await pinHistoricalCommand(port, command.launchContext.id, pinned);
      await history.refresh();
      notify(
        pinned
          ? `Pinned default command for :${port}.`
          : `Unpinned command from :${port}.`,
      );
    } catch (reason) {
      notify(String(reason), true);
    }
  }

  if (!favorites.length)
    return (
      <section className="panel empty-state favorites-empty">
        <Star size={28} />
        <h3>No favorite ports yet.</h3>
        <p>Star a port to turn its activity into a reusable launch point.</p>
      </section>
    );

  return (
    <>
      <div className="favorite-memory-grid">
        {favorites.map((port) => {
          const active = ports.find((entry) => entry.port === port);
          const commands = (history.data?.commands ?? []).filter((command) =>
            command.typicalPorts.includes(port),
          );
          const pinned = commands.find((command) =>
            command.pinnedPorts.includes(port),
          );
          const primary = pinned ?? commands[0];
          const showHistory = expanded === port;
          return (
            <section
              className={`panel favorite-memory-card ${active ? "" : "favorite-available"}`}
              key={port}
            >
              <header>
                <button
                  className="star-button is-favorite"
                  aria-label={`Unwatch port ${port}`}
                  onClick={() => toggleFavorite(port)}
                >
                  <Star size={16} fill="currentColor" />
                </button>
                <strong className="mono">:{port}</strong>
                <span className={`watch-badge ${active ? "in-use" : ""}`}>
                  {active ? "IN USE" : paused ? "LAST SEEN FREE" : "AVAILABLE"}
                </span>
              </header>
              {active ? (
                <div className="favorite-current-owner">
                  <span className="section-label">CURRENT OWNER</span>
                  <h3>{active.project?.name ?? active.process}</h3>
                  <p>{active.serviceName ?? active.process}</p>
                  <code>
                    {active.launch?.command ?? active.command.join(" ")}
                  </code>
                  <div className="favorite-actions">
                    <button
                      className="button primary"
                      onClick={() => inspect(active)}
                    >
                      Open
                    </button>
                    {active.restartable && (
                      <button
                        className="button"
                        onClick={() => requestAction(active, "restart")}
                      >
                        Restart
                      </button>
                    )}
                    <button
                      className="button"
                      onClick={() =>
                        setExpanded(showHistory ? undefined : port)
                      }
                    >
                      <History size={13} /> History
                    </button>
                  </div>
                  {primary?.active && (
                    <p className="duplicate-note">
                      This command appears to already be running. Use Open or
                      Restart.
                    </p>
                  )}
                </div>
              ) : primary ? (
                <div className="favorite-last-command">
                  <span className="section-label">
                    {pinned ? "PINNED COMMAND" : "LAST COMMAND"}
                  </span>
                  <h3>
                    {primary.latestRun.projectName ??
                      primary.latestRun.processName ??
                      primary.launchContext.projectId
                        ?.split(/[\\/]/)
                        .filter(Boolean)
                        .at(-1) ??
                      "Recovered command"}
                  </h3>
                  <code>{primary.launchContext.command}</code>
                  <p>
                    {primary.runCount
                      ? `Last run ${relative(primary.latestRun.startedAt)}`
                      : "Saved launch context"}
                    {commands.length > 1
                      ? ` · ${commands.length} historical commands`
                      : ""}
                  </p>
                  <div className="favorite-actions">
                    {primary.launchContext.recoverable ? (
                      <button
                        className="button primary"
                        disabled={busy === primary.launchContext.id}
                        onClick={() => void run(primary)}
                      >
                        <Play size={13} fill="currentColor" />
                        {busy === primary.launchContext.id
                          ? "Starting…"
                          : "Run Again"}
                      </button>
                    ) : (
                      <span className="command-unavailable">
                        Command unavailable
                      </span>
                    )}
                    <button
                      className="button"
                      onClick={() =>
                        setExpanded(showHistory ? undefined : port)
                      }
                    >
                      <History size={13} /> History
                    </button>
                  </div>
                </div>
              ) : (
                <div className="favorite-no-history">
                  <Clock3 size={20} />
                  <div>
                    <h3>No command history yet</h3>
                    <p>
                      Port Authority will remember the next captured command
                      that uses :{port}.
                    </p>
                  </div>
                  <button className="button" onClick={() => openTimeline(port)}>
                    Port Timeline <ExternalLink size={13} />
                  </button>
                </div>
              )}
              {showHistory && (
                <div className="favorite-command-history">
                  <div className="favorite-history-heading">
                    <span className="section-label">RECENT COMMANDS</span>
                    <button
                      className="text-button"
                      onClick={() => openTimeline(port)}
                    >
                      Ownership history <ExternalLink size={12} />
                    </button>
                  </div>
                  {commands.length ? (
                    commands.map((command) => (
                      <div
                        className="favorite-command-row"
                        key={command.launchContext.id}
                      >
                        <div>
                          <strong>
                            {command.latestRun.projectName ??
                              command.latestRun.processName ??
                              command.launchContext.projectId
                                ?.split(/[\\/]/)
                                .filter(Boolean)
                                .at(-1) ??
                              "Recovered command"}
                          </strong>
                          <code>{command.launchContext.command}</code>
                          <span>
                            {command.runCount
                              ? `${relative(command.latestRun.startedAt)} · ${command.runCount} run${command.runCount === 1 ? "" : "s"}`
                              : "Saved command"}
                          </span>
                        </div>
                        <div>
                          {!active && command.launchContext.recoverable && (
                            <button
                              className="button"
                              disabled={busy === command.launchContext.id}
                              onClick={() => void run(command)}
                            >
                              Run
                            </button>
                          )}
                          <button
                            className={`icon-button ${command.pinnedPorts.includes(port) ? "is-pinned" : ""}`}
                            title={
                              command.pinnedPorts.includes(port)
                                ? "Unpin default command"
                                : "Pin as default command"
                            }
                            onClick={() => void pin(port, command)}
                          >
                            <Pin
                              size={14}
                              fill={
                                command.pinnedPorts.includes(port)
                                  ? "currentColor"
                                  : "none"
                              }
                            />
                          </button>
                        </div>
                      </div>
                    ))
                  ) : (
                    <p className="muted">
                      No captured launch context is associated with this port.
                    </p>
                  )}
                </div>
              )}
            </section>
          );
        })}
      </div>
      {confirmation && (
        <RunAgainDialog
          command={confirmation}
          close={() => setConfirmation(null)}
          confirm={() => {
            const command = confirmation;
            setConfirmation(null);
            void run(command, true);
          }}
        />
      )}
    </>
  );
}

function relative(timestamp: number) {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 172800) return "yesterday";
  return new Date(timestamp * 1000).toLocaleDateString([], {
    month: "short",
    day: "numeric",
  });
}
