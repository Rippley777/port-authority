import { memo, useEffect, useMemo, useState } from "react";
import {
  History,
  Search,
  RotateCw,
  ArrowRight,
  Circle,
  Star,
} from "lucide-react";
import type { PortEntry } from "../../lib/types";
import { useTimeline } from "./useTimeline";
import { eventLabels, type TimelineEvent, type Observation } from "./types";
import { RecurrencePanel } from "./RecurrencePanel";
import { ProjectActions } from "../projects/ProjectActions";
import { ownershipSessions } from "../../lib/ownership";
import { TimelineStorageControls } from "./TimelineSettings";
const time = (at: number) =>
  new Date(at * 1000).toLocaleTimeString([], { hour12: false });
const day = (at: number) =>
  new Date(at * 1000).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
export function TimelinePage({
  port,
  ports,
  favorites,
  paused,
  failed,
  selectPort,
  inspect,
  toggleFavorite,
}: {
  port?: number;
  ports: PortEntry[];
  favorites: number[];
  paused: boolean;
  failed: boolean;
  selectPort: (port?: number) => void;
  inspect: (p: PortEntry) => void;
  toggleFavorite: (port: number) => void;
}) {
  const [portText, setPortText] = useState(port ? String(port) : "");
  const [search, setSearch] = useState("");
  const [type, setType] = useState("");
  const [range, setRange] = useState("86400");
  const [tab, setTab] = useState("events");
  useEffect(() => setPortText(port ? String(port) : ""), [port]);
  const since = useMemo(
    () =>
      range === "all"
        ? undefined
        : range === "today"
          ? Math.floor(new Date().setHours(0, 0, 0, 0) / 1000)
          : Math.floor(Date.now() / 1000) - Number(range),
    [range],
  );
  const timeline = useTimeline({
    port,
    search: tab === "events" ? search : undefined,
    eventType: tab === "events" ? type || undefined : undefined,
    since,
  });
  const { data } = timeline;
  const live = ports.filter((p) => p.port === port);
  const sessions = useMemo(
    () => ownershipSessions(data?.events ?? [], data?.sessions ?? []),
    [data],
  );
  const displayedEvents = useMemo(() => {
    const events = data?.events ?? [];
    if (type || search) return events;
    const correlated = new Set(
      events.map((e) => e.correlationId).filter(Boolean),
    );
    return events.filter((e) => !correlated.has(e.id));
  }, [data, type, search]);
  const invalid =
    portText !== "" &&
    (!/^\d+$/.test(portText) ||
      Number(portText) < 1 ||
      Number(portText) > 65535);
  return (
    <div className="timeline-page">
      <section className="panel timeline-toolbar">
        <div className="timeline-intro">
          <History size={22} />
          <div>
            <h2>Port Timeline</h2>
            <p>A local flight recorder for your development ports.</p>
          </div>
          <span className="timeline-status">
            <span
              className={`status-dot ${paused || failed || !data?.monitoring ? "paused" : ""}`}
            />
            {paused
              ? "Monitoring paused"
              : failed
                ? "Scan unavailable"
                : data?.monitoring
                  ? "Recording transitions"
                  : "Recording inactive"}
          </span>
        </div>
        <div className="timeline-filters">
          <label>
            Port
            <input
              aria-label="Timeline port"
              inputMode="numeric"
              placeholder="All ports"
              value={portText}
              onChange={(e) => {
                const value = e.target.value;
                setPortText(value);
                if (value === "") selectPort(undefined);
                else if (
                  /^\d+$/.test(value) &&
                  Number(value) > 0 &&
                  Number(value) <= 65535
                )
                  selectPort(Number(value));
              }}
            />
          </label>
          <label className="timeline-search">
            Search history
            <div>
              <Search size={15} />
              <input
                aria-label="Search history"
                placeholder="Project, process, PID 48291…"
                value={search}
                disabled={tab !== "events"}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          </label>
          <label>
            Event
            <select
              aria-label="Timeline event type"
              value={type}
              disabled={tab !== "events"}
              onChange={(e) => setType(e.target.value)}
            >
              <option value="">All events</option>
              {Object.entries(eventLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Time range
            <select
              aria-label="Timeline time range"
              value={range}
              onChange={(e) => setRange(e.target.value)}
            >
              <option value="3600">Last hour</option>
              <option value="today">Today</option>
              <option value="86400">24 hours</option>
              <option value="604800">7 days</option>
              <option value="2592000">30 days</option>
              <option value="all">All retained history</option>
            </select>
          </label>
        </div>
        {invalid && <p role="alert">Enter a port between 1 and 65535.</p>}
        <div className="timeline-actions">
          <button
            className="text-button"
            onClick={() => {
              selectPort(undefined);
              setPortText("");
              setSearch("");
              setType("");
            }}
          >
            All ports
          </button>
          {favorites.map((p) => (
            <button
              className={`timeline-port-chip ${port === p ? "selected" : ""}`}
              key={p}
              onClick={() => selectPort(p)}
            >
              <Star size={11} />
              {p}
            </button>
          ))}
          <span className="muted">
            Watched ports record the same detailed history.
          </span>
        </div>
      </section>
      {timeline.error && (
        <div className="error-banner" role="alert">
          <p>{timeline.error}</p>
          <button className="button" onClick={() => void timeline.refresh()}>
            Retry
          </button>
        </div>
      )}
      {port && (
        <section className="panel timeline-current">
          <div className="timeline-current-heading">
            <span className="section-label">
              {paused || failed ? "LAST OBSERVED" : "CURRENTLY"}
            </span>
            <strong className="mono">:{port}</strong>
            <button
              className="icon-button"
              onClick={() => toggleFavorite(port)}
              aria-label={
                favorites.includes(port)
                  ? "Unwatch timeline port"
                  : "Watch timeline port"
              }
            >
              <Star
                size={16}
                fill={favorites.includes(port) ? "currentColor" : "none"}
              />
            </button>
          </div>
          {live.length ? (
            live.map((p) => (
              <div className="timeline-current-owner" key={p.id}>
                <div>
                  <strong>{p.project?.name ?? p.process}</strong>
                  <p>
                    {p.serviceName ?? p.process} · PID {p.pid ?? "unavailable"}{" "}
                    · {p.protocol} · {p.address}
                  </p>
                  <small>{p.project?.displayPath ?? p.cwd}</small>
                </div>
                <button className="button" onClick={() => inspect(p)}>
                  Inspect Process
                  <ArrowRight size={13} />
                </button>
              </div>
            ))
          ) : (
            <p>
              {paused || failed
                ? "Current availability is unknown."
                : "No listener in the latest scan."}
            </p>
          )}
        </section>
      )}
      {data?.recurring.map((r) => (
        <RecurrencePanel
          key={`${r.port}:${r.protocol}:${r.address}`}
          recurrence={r}
        />
      ))}
      <section className="panel timeline-events">
        <div className="panel-heading">
          <div>
            <h3>{port ? `History of :${port}` : "Across all ports"}</h3>
            <span className="count-badge">{displayedEvents.length}</span>
          </div>
          <div
            className="timeline-tabs"
            role="tablist"
            aria-label="History view"
          >
            <button
              role="tab"
              aria-selected={tab === "events"}
              onClick={() => setTab("events")}
            >
              Events
            </button>
            <button
              role="tab"
              aria-selected={tab === "sessions"}
              onClick={() => setTab("sessions")}
            >
              Ownership sessions
            </button>
          </div>
        </div>
        <p className="timeline-observation-note">
          Times are observations, to the nearest scan. Activity between scans
          may be missed. Correlated releases are grouped into their restart; use
          the release filter to see raw events.
        </p>
        {!!data?.sessions.length && (
          <details className="timeline-observation-note monitoring-details">
            <summary>
              Monitoring sessions · unknown periods are not availability
            </summary>
            {data.sessions.slice(0, 10).map((session, index) => (
              <div key={session.id}>
                <p>
                  <strong>Monitoring resumed</strong>{" "}
                  {new Date(session.startedAt * 1000).toLocaleString()}
                  <br />
                  {session.endedAt
                    ? "Monitoring stopped"
                    : "Last observed"}{" "}
                  {new Date(session.lastObserved * 1000).toLocaleString()} ·{" "}
                  {session.reason}
                </p>
                {index < Math.min(data.sessions.length, 10) - 1 && (
                  <p className="muted">
                    Monitoring unavailable between these sessions. A clock
                    adjustment may also change displayed times.
                  </p>
                )}
              </div>
            ))}
          </details>
        )}
        {timeline.pending && (
          <button
            className="button timeline-new"
            onClick={timeline.showPending}
          >
            Show new activity
          </button>
        )}
        {data && !data.config.enabled && (
          <p className="timeline-observation-note">
            History recording is disabled. Existing records remain available.
          </p>
        )}
        {!data && timeline.loading ? (
          <div className="empty-state">
            <RotateCw size={23} className="spin" />
            <p>Reading local history…</p>
          </div>
        ) : data?.events.length ? (
          <div className="history-list port-timeline-list">
            {tab === "events"
              ? displayedEvents.map((event, i) => {
                  const newer = displayedEvents[i - 1];
                  const session = data.sessions.find(
                    (s) => s.id === event.sessionId,
                  );
                  return (
                    <div key={event.id}>
                      {(!newer ||
                        day(newer.timestamp) !== day(event.timestamp)) && (
                        <h4 className="timeline-day">{day(event.timestamp)}</h4>
                      )}
                      {newer && newer.sessionId !== event.sessionId && (
                        <div className="timeline-gap">
                          <span>Monitoring unavailable</span>
                          <p>
                            After{" "}
                            {session
                              ? new Date(
                                  session.lastObserved * 1000,
                                ).toLocaleString()
                              : "the last retained observation"}{" "}
                            until{" "}
                            {new Date(
                              (data.sessions.find(
                                (s) => s.id === newer.sessionId,
                              )?.startedAt ?? newer.timestamp) * 1000,
                            ).toLocaleString()}
                            . Changes in this interval have no exact time.
                          </p>
                        </div>
                      )}
                      <EventRow event={event} selectPort={selectPort} />
                    </div>
                  );
                })
              : sessions.map((s) => (
                  <div className="ownership-row" key={s.id}>
                    <span className="mono">
                      {time(s.startedAt)} → {time(s.endedAt)}
                    </span>
                    <div>
                      <strong>
                        {s.owner?.process.project?.name ??
                          s.owner?.process.serviceName ??
                          s.owner?.process.process ??
                          "Available"}
                      </strong>
                      <p>
                        {s.endpoint}
                        {s.owner
                          ? ` · PID ${s.owner.process.pid ?? "unknown"}`
                          : ""}
                      </p>
                      <small>
                        {s.uncertainStart
                          ? "Already present when monitoring resumed. "
                          : ""}
                        Interval bounded by observed events.
                      </small>
                    </div>
                  </div>
                ))}
          </div>
        ) : (
          <div className="empty-state">
            <History size={28} />
            <h3>No matching activity yet.</h3>
            <p>
              {search || type
                ? "Try another search or event filter."
                : "Claims, releases, and restarts will appear as port ownership changes."}
            </p>
          </div>
        )}
        {data?.nextCursor && (
          <div className="timeline-load">
            <button
              className="button"
              disabled={timeline.loading}
              onClick={() => void timeline.more()}
            >
              Load older events
            </button>
          </div>
        )}
      </section>
      {data && <TimelineStorageControls data={data} reload={timeline.reset} />}
    </div>
  );
}
const EventRow = memo(function EventRow({
  event: e,
  selectPort,
}: {
  event: TimelineEvent;
  selectPort: (p: number) => void;
}) {
  const p = (e.process ?? e.previousProcess)?.process;
  return (
    <article className={`timeline-event ${e.eventType.toLowerCase()}`}>
      <time
        className="mono"
        title={new Date(e.timestamp * 1000).toLocaleString()}
      >
        {time(e.timestamp)}
      </time>
      <span className="timeline-marker">
        {e.eventType === "PROCESS_RESTARTED" ? (
          <RotateCw size={14} />
        ) : (
          <Circle
            size={10}
            fill={e.eventType === "PORT_RELEASED" ? "none" : "currentColor"}
          />
        )}
      </span>
      <div className="timeline-event-body">
        <div className="timeline-event-heading">
          <strong>
            {e.uncertain
              ? e.eventType === "OWNER_CHANGED"
                ? "Owner changed while monitoring was offline"
                : e.eventType === "PORT_RELEASED"
                  ? "No longer present when monitoring resumed"
                  : "Owner observed when monitoring resumed"
              : eventLabels[e.eventType]}
          </strong>
          <button
            className="timeline-port-chip mono"
            onClick={() => selectPort(e.port)}
          >
            :{e.port}
          </button>
        </div>
        <p>
          {p?.project?.name ??
            p?.serviceName ??
            p?.process ??
            "Owner unavailable"}
          {p?.project ? ` · ${p.serviceName ?? p.process}` : ""}
        </p>
        <div className="timeline-event-meta">
          <span>
            {e.protocol} · {e.address}
          </span>
          <span className="mono">
            {e.previousProcess && e.process
              ? `PID ${e.previousProcess.process.pid ?? "?"} → ${e.process.process.pid ?? "?"}`
              : `PID ${p?.pid ?? "unavailable"}`}
          </span>
          {p?.project && <span>{p.project.displayPath}</span>}
        </div>
        {e.uncertain && (
          <small className="muted">
            Transition time unknown; this is the first observation after a
            monitoring gap.
          </small>
        )}
        <details>
          <summary>
            Snapshot details
            {e.correlationId ? " · correlated with the preceding release" : ""}
          </summary>
          {e.previousProcess && e.process && (
            <Snapshot observation={e.previousProcess} label="Previous owner" />
          )}
          {(e.process ?? e.previousProcess) && (
            <Snapshot
              observation={(e.process ?? e.previousProcess)!}
              label={e.process ? "Observed owner" : "Released owner"}
            />
          )}
        </details>
      </div>
    </article>
  );
});
function Snapshot({
  observation: o,
  label,
}: {
  observation: Observation;
  label: string;
}) {
  const p = o.process;
  return (
    <div className="timeline-snapshot">
      <h4>
        {label} · {p.project?.name ?? p.process}
      </h4>
      <dl>
        <dt>Process identity</dt>
        <dd>
          PID {p.pid ?? "unknown"} · started{" "}
          {p.startedAt
            ? new Date(p.startedAt * 1000).toLocaleString()
            : "unknown"}
        </dd>
        <dt>Command</dt>
        <dd>
          <code>{p.command.join(" ") || "Unavailable"}</code>
        </dd>
        <dt>Executable</dt>
        <dd>{p.executable ?? "Unavailable"}</dd>
        <dt>Working directory</dt>
        <dd>{p.cwd ?? "Unavailable"}</dd>
        <dt>User / parent</dt>
        <dd>
          {p.user ?? "Unknown"} · PID {p.parentPid ?? "unknown"}
        </dd>
        {p.project?.repository && (
          <>
            <dt>Repository</dt>
            <dd>
              {p.project.repository.owner}/{p.project.repository.repository}
            </dd>
          </>
        )}
      </dl>
      {o.ancestors.length > 0 && (
        <div className="timeline-ancestors">
          <strong>Observed ancestry</strong>
          {o.ancestors.map((a) => (
            <p key={`${a.identity.pid}:${a.identity.startedAt}`}>
              ↑ {a.name} · PID {a.identity.pid}
              <code>{a.command.join(" ")}</code>
            </p>
          ))}
        </div>
      )}
      {p.project && <ProjectActions project={p.project} />}
    </div>
  );
}
