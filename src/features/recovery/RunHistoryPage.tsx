import { invoke } from "@tauri-apps/api/core";
import {
  CheckCircle2,
  CircleStop,
  Clock3,
  Copy,
  ExternalLink,
  History,
  Play,
  Search,
  Terminal,
  XCircle,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { PortEntry } from "../../lib/types";
import { desktop } from "../../lib/api";
import { Modal } from "../../components/Modal";
import { useProjectContext } from "../projects/useProjects";
import { RecoveryProgress } from "./LaunchSection";
import {
  historicalCommandNeedsConfirmation,
  pinHistoricalCommand,
  removeHistoricalRun,
  runHistoricalCommand,
} from "./api";
import { RunAgainDialog } from "./RunAgainDialog";
import type { CommandRun, HistoricalCommand } from "./types";
import { useRunHistory } from "./useRunHistory";

const relative = (timestamp: number) => {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 172800) return "Yesterday";
  return new Date(timestamp * 1000).toLocaleDateString([], {
    month: "short",
    day: "numeric",
  });
};
const duration = (run: CommandRun) => {
  const seconds =
    (run.endedAt ?? Math.floor(Date.now() / 1000)) - run.startedAt;
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
};
const stateLabel: Record<CommandRun["state"], string> = {
  RUNNING: "Currently running",
  COMPLETED: "Completed",
  FAILED: "Failed",
  STOPPED: "Stopped",
  UNVERIFIED: "Unverified",
};

export function RunHistoryPage({
  port,
  ports,
  favorites,
  inspect,
  openTimeline,
  copy,
  refreshPorts,
  notify,
}: {
  port?: number;
  ports: PortEntry[];
  favorites: number[];
  inspect: (entry: PortEntry) => void;
  openTimeline: (port?: number) => void;
  copy: (value: string, label: string) => void;
  refreshPorts: () => Promise<unknown>;
  notify: (message: string, error?: boolean) => void;
}) {
  const [search, setSearch] = useState("");
  const [state, setState] = useState("");
  const [range, setRange] = useState("2592000");
  const [selected, setSelected] = useState<HistoricalCommand | null>(null);
  const [confirmation, setConfirmation] = useState<HistoricalCommand | null>(
    null,
  );
  const [busy, setBusy] = useState<string>();
  const [feedback, setFeedback] = useState<Record<string, string>>({});
  const projects = useProjectContext();
  const since =
    range === "all"
      ? undefined
      : range === "today"
        ? Math.floor(new Date().setHours(0, 0, 0, 0) / 1000)
        : Math.floor(Date.now() / 1000) - Number(range);
  const history = useRunHistory({
    search: search || undefined,
    state: state || undefined,
    since,
    port,
    limit: 500,
  });
  const commands = history.data?.commands ?? [];
  const recentlyRun = useMemo(() => commands.slice(0, 3), [commands]);
  const pinPort = selected?.typicalPorts.find((candidate) =>
    favorites.includes(candidate),
  );

  async function run(command: HistoricalCommand, confirmed = false) {
    if (
      historicalCommandNeedsConfirmation(command.launchContext) &&
      !confirmed
    ) {
      setConfirmation(command);
      return;
    }
    const id = command.launchContext.id;
    setBusy(id);
    setFeedback((current) => ({ ...current, [id]: "Starting…" }));
    try {
      const result = await runHistoricalCommand(id, confirmed);
      setFeedback((current) => ({ ...current, [id]: result.message }));
      notify(result.message);
      await Promise.all([history.refresh(), refreshPorts()]);
    } catch (reason) {
      const message = String(reason);
      setFeedback((current) => ({ ...current, [id]: message }));
      notify(message, true);
    } finally {
      setBusy(undefined);
    }
  }

  return (
    <div className="run-history-page">
      {!port && recentlyRun.length > 0 && (
        <section className="run-recent-section">
          <div className="run-section-heading">
            <div>
              <span className="section-label">RECENTLY RUN</span>
              <h2>Pick up where you left off</h2>
            </div>
          </div>
          <div className="run-recent-grid">
            {recentlyRun.map((command) => (
              <CommandCard
                key={
                  command.launchContext.fingerprint ?? command.launchContext.id
                }
                command={command}
                compact
                busy={busy === command.launchContext.id}
                feedback={feedback[command.launchContext.id]}
                run={() => void run(command)}
                details={() => setSelected(command)}
                open={() => {
                  const active = activeEntry(command, ports);
                  if (active) inspect(active);
                }}
              />
            ))}
          </div>
        </section>
      )}
      <section className="panel run-history-panel">
        <div className="run-history-toolbar">
          <label className="search-field">
            <Search size={16} />
            <input
              aria-label="Search run history"
              placeholder="Search projects, commands, directories, or ports…"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <select
            aria-label="Run status"
            value={state}
            onChange={(event) => setState(event.target.value)}
          >
            <option value="">All outcomes</option>
            <option value="RUNNING">Running</option>
            <option value="COMPLETED">Completed</option>
            <option value="FAILED">Failed</option>
            <option value="STOPPED">Stopped</option>
          </select>
          <select
            aria-label="Run history time range"
            value={range}
            onChange={(event) => setRange(event.target.value)}
          >
            <option value="today">Today</option>
            <option value="86400">24 hours</option>
            <option value="604800">7 days</option>
            <option value="2592000">30 days</option>
            <option value="all">All retained history</option>
          </select>
        </div>
        {history.error && (
          <div className="error-banner" role="alert">
            {history.error}
          </div>
        )}
        <div className="run-history-heading">
          <div>
            <h3>{port ? `Recent commands on :${port}` : "Command history"}</h3>
            <span className="count-badge">{commands.length}</span>
          </div>
          {!port && (
            <button className="text-button" onClick={() => openTimeline()}>
              Port Timeline <ExternalLink size={13} />
            </button>
          )}
        </div>
        {history.loading && !history.data ? (
          <div className="empty-state">
            <Clock3 size={24} />
            <p>Reading command history…</p>
          </div>
        ) : commands.length ? (
          <div className="command-history-list">
            {commands.map((command) => (
              <CommandCard
                key={
                  command.launchContext.fingerprint ?? command.launchContext.id
                }
                command={command}
                busy={busy === command.launchContext.id}
                feedback={feedback[command.launchContext.id]}
                run={() => void run(command)}
                details={() => setSelected(command)}
                open={() => {
                  const active = activeEntry(command, ports);
                  if (active) inspect(active);
                }}
              />
            ))}
          </div>
        ) : (
          <div className="empty-state">
            <History size={28} />
            <h3>No matching commands yet.</h3>
            <p>
              {search || state
                ? "Try another search or outcome filter."
                : "Commands captured by shell integration will become reusable launch points here."}
            </p>
          </div>
        )}
      </section>
      {selected && (
        <RunDetails
          command={selected}
          runs={(history.data?.runs ?? []).filter(
            (run) => run.fingerprint === selected.latestRun.fingerprint,
          )}
          close={() => setSelected(null)}
          run={() => void run(selected)}
          copy={copy}
          openTimeline={openTimeline}
          openProject={(() => {
            const project = projects.projects.find(
              (candidate) =>
                candidate.identity.rootPath ===
                selected.launchContext.projectId,
            );
            return project
              ? () => void projects.action(project.identity, "editor")
              : undefined;
          })()}
          pin={
            pinPort
              ? async () => {
                  const favoritePort = pinPort;
                  const pinned = !selected.pinnedPorts.includes(favoritePort);
                  try {
                    await pinHistoricalCommand(
                      favoritePort,
                      selected.launchContext.id,
                      pinned,
                    );
                    await history.refresh();
                    setSelected((current) =>
                      current
                        ? {
                            ...current,
                            pinnedPorts: pinned ? [favoritePort] : [],
                          }
                        : current,
                    );
                    notify(
                      pinned
                        ? `Pinned default command for :${favoritePort}.`
                        : `Unpinned command from :${favoritePort}.`,
                    );
                  } catch (reason) {
                    notify(String(reason), true);
                  }
                }
              : undefined
          }
          removed={async (id) => {
            try {
              await removeHistoricalRun(id);
              await history.refresh();
              setSelected(null);
              notify("Run removed from history.");
            } catch (reason) {
              notify(String(reason), true);
            }
          }}
        />
      )}
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
    </div>
  );
}

function activeEntry(command: HistoricalCommand, ports: PortEntry[]) {
  return ports.find(
    (entry) =>
      entry.launch?.id === command.launchContext.id ||
      (entry.project?.rootPath === command.launchContext.projectId &&
        command.typicalPorts.includes(entry.port)),
  );
}

function CommandCard({
  command,
  compact = false,
  busy,
  feedback,
  run,
  details,
  open,
}: {
  command: HistoricalCommand;
  compact?: boolean;
  busy: boolean;
  feedback?: string;
  run: () => void;
  details: () => void;
  open: () => void;
}) {
  const context = command.launchContext;
  const latest = command.latestRun;
  const projectName =
    latest.projectName ??
    context.projectId?.split(/[\\/]/).filter(Boolean).at(-1) ??
    latest.processName ??
    "Recovered command";
  return (
    <article
      className={`command-history-card ${compact ? "compact" : ""}`}
      tabIndex={0}
      aria-label={`${projectName}: ${context.command}`}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          if (!command.active && context.recoverable && !busy) {
            event.preventDefault();
            run();
          }
        } else if (event.key === "Enter") {
          event.preventDefault();
          details();
        }
      }}
    >
      <div className="command-history-project">
        <span className={`run-state ${latest.state.toLowerCase()}`}>
          {latest.state === "RUNNING" ? (
            <span className="status-dot" />
          ) : latest.state === "FAILED" ? (
            <XCircle size={14} />
          ) : latest.state === "COMPLETED" ? (
            <CheckCircle2 size={14} />
          ) : (
            <CircleStop size={14} />
          )}
          {projectName}
        </span>
        <span className="muted">{relative(latest.startedAt)}</span>
      </div>
      <strong className="mono command-history-command">
        {context.command}
      </strong>
      <span className="command-history-directory">
        {context.workingDirectory}
      </span>
      <div className="command-history-meta">
        {command.typicalPorts.map((port) => (
          <span className="mono" key={port}>
            :{port}
          </span>
        ))}
        <span>
          {command.runCount ? stateLabel[latest.state] : "Saved command"}
        </span>
        {command.runCount > 0 && <span>{duration(latest)}</span>}
        {command.runCount > 1 && <span>{command.runCount} runs</span>}
      </div>
      {feedback && (
        <p className="run-feedback" role="status">
          {feedback}
        </p>
      )}
      <div className="command-history-actions">
        {command.active ? (
          <button className="button primary" onClick={open}>
            Open
          </button>
        ) : context.recoverable ? (
          <button className="button primary" disabled={busy} onClick={run}>
            <Play size={13} fill="currentColor" />
            {busy ? "Starting…" : "Run Again"}
          </button>
        ) : (
          <span className="command-unavailable" title={context.reason}>
            Command unavailable
          </span>
        )}
        <button className="button" onClick={details}>
          Details
        </button>
      </div>
    </article>
  );
}

function RunDetails({
  command,
  runs,
  close,
  run,
  copy,
  openTimeline,
  openProject,
  pin,
  removed,
}: {
  command: HistoricalCommand;
  runs: CommandRun[];
  close: () => void;
  run: () => void;
  copy: (value: string, label: string) => void;
  openTimeline: (port?: number) => void;
  openProject?: () => void;
  pin?: () => Promise<void>;
  removed: (id: string) => Promise<void>;
}) {
  const context = command.launchContext;
  const [message, setMessage] = useState("");
  return (
    <Modal close={close} label={`Run details for ${context.command}`}>
      <div className="run-details-header">
        <div>
          <span className="section-label">
            {command.latestRun.projectName ?? "COMMAND"}
          </span>
          <h2 className="mono">{context.command}</h2>
          <p>{context.workingDirectory}</p>
        </div>
      </div>
      <dl className="details run-details-grid">
        <dt>Last started</dt>
        <dd>{new Date(command.latestRun.startedAt * 1000).toLocaleString()}</dd>
        <dt>Last stopped</dt>
        <dd>
          {command.latestRun.endedAt
            ? new Date(command.latestRun.endedAt * 1000).toLocaleString()
            : "Still running"}
        </dd>
        <dt>Duration</dt>
        <dd>{duration(command.latestRun)}</dd>
        <dt>Ports</dt>
        <dd className="mono">
          {command.typicalPorts.map((port) => `:${port}`).join(", ") ||
            "None observed"}
        </dd>
        <dt>Process</dt>
        <dd>{command.latestRun.processName ?? "Unavailable"}</dd>
        <dt>Exit</dt>
        <dd>
          {command.latestRun.exitCode !== null
            ? `Exit code ${command.latestRun.exitCode}`
            : (command.latestRun.terminationReason ??
              stateLabel[command.latestRun.state])}
        </dd>
      </dl>
      {!context.recoverable && (
        <p className="drawer-footnote">{context.reason}</p>
      )}
      <div className="recovery-actions run-details-actions">
        {!command.active && context.recoverable && (
          <button className="button primary" onClick={run}>
            <Play size={13} fill="currentColor" /> Run Again
          </button>
        )}
        <button
          className="button"
          onClick={() => copy(context.command, "Command")}
        >
          <Copy size={13} /> Copy Command
        </button>
        <button
          className="button"
          disabled={!desktop}
          onClick={() => {
            void invoke("recovery_terminal", {
              directory: context.workingDirectory,
            })
              .then(() => setMessage("Terminal opened."))
              .catch((reason) => setMessage(String(reason)));
          }}
        >
          <Terminal size={13} /> Open Terminal Here
        </button>
        {openProject && (
          <button className="button" onClick={openProject}>
            Open Project
          </button>
        )}
        <button
          className="button"
          onClick={() => copy(context.workingDirectory, "Working directory")}
        >
          Copy Working Directory
        </button>
        {command.typicalPorts[0] && (
          <button
            className="button"
            onClick={() => {
              close();
              openTimeline(command.typicalPorts[0]);
            }}
          >
            View Port Timeline
          </button>
        )}
        {pin && (
          <button className="button" onClick={() => void pin()}>
            {command.pinnedPorts.includes(command.typicalPorts[0])
              ? "Unpin Default Command"
              : "Pin as Default Command"}
          </button>
        )}
      </div>
      <RecoveryProgress id={context.id} />
      {message && <p role="status">{message}</p>}
      {runs.length === 1 && (
        <button
          className="text-button danger"
          onClick={() => void removed(command.latestRun.id)}
        >
          Remove From History
        </button>
      )}
      {runs.length > 1 && (
        <section className="run-instances">
          <h3>Individual runs</h3>
          {runs.map((run) => (
            <div key={run.id} className="run-instance-row">
              <span>{new Date(run.startedAt * 1000).toLocaleString()}</span>
              <span>
                {stateLabel[run.state]} · {duration(run)}
              </span>
              <button
                className="text-button danger"
                onClick={() => void removed(run.id)}
              >
                Remove
              </button>
            </div>
          ))}
        </section>
      )}
    </Modal>
  );
}
