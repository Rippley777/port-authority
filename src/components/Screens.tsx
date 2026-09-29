import { ProjectPreferences } from "../features/projects/ProjectPreferences";
import {
  ArrowDownLeft,
  ArrowRight,
  ArrowUpRight,
  Check,
  Copy,
  Eye,
  History,
  Info,
  Network,
  Search,
  Shield,
  Star,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import type { HistoryEvent, PortEntry, Settings } from "../lib/types";
import { portFromQuery, service, uptime } from "../lib/ports";
import { ServiceIcon } from "./ServiceIcon";
import { desktop } from "../lib/api";
export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onChange: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      className={`toggle ${checked ? "checked" : ""}`}
      role="switch"
      aria-label={label}
      aria-checked={checked}
      onClick={onChange}
      disabled={disabled}
    >
      <span />
    </button>
  );
}
export function Stats({ ports }: { ports: PortEntry[] }) {
  const values = [
    {
      label: "Active ports",
      value: ports.length,
      icon: Network,
      note: "Across all interfaces",
      className: "mint",
    },
    {
      label: "User processes",
      value: new Set(ports.filter((p) => !p.system && p.pid).map((p) => p.pid))
        .size,
      icon: SquareIcon,
      note: "Running on your machine",
      className: "",
    },
    {
      label: "TCP listeners",
      value: ports.filter((p) => p.protocol === "TCP").length,
      icon: ArrowUpRight,
      note: "Connection-oriented",
      className: "",
    },
    {
      label: "UDP sockets",
      value: ports.filter((p) => p.protocol === "UDP").length,
      icon: ArrowDownLeft,
      note: "Connectionless",
      className: "",
    },
  ];
  return (
    <div className="stats-grid">
      {values.map((s) => (
        <div className={`stat ${s.className}`} key={s.label}>
          <div className="stat-label">
            <span>{s.label}</span>
            <s.icon size={15} />
          </div>
          <div className="stat-value">
            {s.value.toString().padStart(2, "0")}
            <span>{s.note}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
function SquareIcon({ size }: { size: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 18 18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
    >
      <rect x="3" y="3" width="12" height="12" rx="2" />
      <path d="m6 6 3 3-3 3m4 0h2" />
    </svg>
  );
}
export function Availability({
  port,
  favorites,
  toggleFavorite,
  copy,
  stale = false,
}: {
  port: number;
  favorites: number[];
  toggleFavorite: (p: number) => void;
  copy: (t: string, l: string) => void;
  stale?: boolean;
}) {
  return (
    <div className="availability">
      <span className="available-icon">
        <Check size={22} />
      </span>
      <div>
        <h3>
          Port <span className="mono">{port}</span>{" "}
          {stale ? "was not in the last scan" : "is available"}
        </h3>
        <p>
          {desktop
            ? "No matching TCP listeners or bound UDP sockets in the latest scan. Availability can change."
            : "No service is using this port in the preview workspace."}
        </p>
      </div>
      <div className="availability-actions">
        <button className="button" onClick={() => copy(String(port), "Port")}>
          <Copy size={14} />
          Copy port
        </button>
        <button className="button primary" onClick={() => toggleFavorite(port)}>
          <Star size={14} />
          {favorites.includes(port) ? "Watching" : "Watch port"}
        </button>
      </div>
    </div>
  );
}
export function CheckPort({
  ports,
  favorites,
  toggleFavorite,
  copy,
  select,
  stale,
}: {
  ports: PortEntry[];
  favorites: number[];
  toggleFavorite: (p: number) => void;
  copy: (t: string, l: string) => void;
  select: (p: PortEntry) => void;
  stale: boolean;
}) {
  const [value, setValue] = useState("");
  const port = portFromQuery(value);
  const matches = ports.filter((p) => p.port === port);
  return (
    <section className="check-panel panel">
      <div className="check-icon">
        <Search size={26} />
      </div>
      <h2>Is your port free?</h2>
      <p>Enter a port to see what’s listening. No terminal required.</p>
      <label className="check-input">
        <span>:</span>
        <input
          inputMode="numeric"
          aria-label="Port number"
          placeholder="5173"
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <kbd>1–65535</kbd>
      </label>
      {value && !port && (
        <p className="form-error">
          Enter a valid port number between 1 and 65535.
        </p>
      )}
      {port &&
        (matches.length ? (
          <div className="check-results">
            {matches.map((p) => (
              <button
                key={p.id}
                className="check-result"
                onClick={() => select(p)}
              >
                <ServiceIcon entry={p} />
                <div>
                  <strong>{p.project?.name ?? p.process}</strong>
                  <small>
                    {p.address} · PID {p.pid ?? "unavailable"}
                  </small>
                </div>
                <span className="listening">
                  <span className="status-dot" />
                  In use
                </span>
                <ArrowRight size={17} />
              </button>
            ))}
          </div>
        ) : (
          <Availability
            port={port}
            favorites={favorites}
            toggleFavorite={toggleFavorite}
            copy={copy}
            stale={stale}
          />
        ))}
      <div className="check-suggestions">
        <span>COMMON PORTS</span>
        {[3000, 5173, 5432, 8080].map((p) => (
          <button className="mono" key={p} onClick={() => setValue(String(p))}>
            {p}
          </button>
        ))}
      </div>
    </section>
  );
}
export function Overview({
  ports,
  favorites,
  select,
  navigateFavorites,
}: {
  ports: PortEntry[];
  favorites: number[];
  select: (p: PortEntry) => void;
  navigateFavorites: () => void;
}) {
  const recent = [...ports]
    .filter((p) => p.startedAt)
    .sort((a, b) => b.startedAt! - a.startedAt!)
    .slice(0, 5);
  return (
    <div className="overview-grid">
      <section className="panel">
        <div className="panel-heading">
          <div>
            <History size={17} />
            <h3>Recently started</h3>
          </div>
          <span className="subtle">Process uptime</span>
        </div>
        {recent.length ? (
          recent.map((p) => (
            <button
              key={p.id}
              className="overview-row"
              onClick={() => select(p)}
            >
              <ServiceIcon entry={p} />
              <span>
                <strong>{p.project?.name ?? p.process}</strong>
                <small>{service(p).label}</small>
              </span>
              <span className="mono">:{p.port}</span>
              <span className="muted">{uptime(p.startedAt)}</span>
              <ArrowUpRight size={15} />
            </button>
          ))
        ) : (
          <div className="empty-state">
            <Network size={26} />
            <h3>Quiet seas.</h3>
            <p>No active services found.</p>
          </div>
        )}
      </section>
      <section className="panel">
        <div className="panel-heading">
          <div>
            <Star size={17} />
            <h3>Watched ports</h3>
          </div>
          <button className="text-button" onClick={navigateFavorites}>
            View all <ArrowRight size={13} />
          </button>
        </div>
        {favorites.length ? (
          favorites.slice(0, 7).map((port) => {
            const p = ports.find((p) => p.port === port);
            return (
              <button
                key={port}
                className="watched-row"
                onClick={() => (p ? select(p) : navigateFavorites())}
              >
                <span className="mono">:{port}</span>
                <span className="muted">
                  {p?.process ?? "Ready when you are"}
                </span>
                <span className={`watch-badge ${p ? "in-use" : ""}`}>
                  {p ? "IN USE" : "AVAILABLE"}
                </span>
              </button>
            );
          })
        ) : (
          <div className="empty-state">
            <Star size={26} />
            <h3>Keep your ports in sight.</h3>
            <p>Star a port to watch its availability here.</p>
          </div>
        )}
      </section>
      <div className="overview-note">
        <Shield size={17} />
        <span>
          A little clarity for your local environment. All monitoring stays on
          this machine.
        </span>
      </div>
    </div>
  );
}
export function HistoryScreen({
  history,
  clear,
  enabled,
}: {
  history: HistoryEvent[];
  clear: () => void;
  enabled: boolean;
}) {
  return (
    <section className="panel history-panel">
      <div className="panel-heading">
        <div>
          <History size={17} />
          <h3>Recent activity</h3>
          <span className="count-badge">{history.length}</span>
        </div>
        <button className="button" onClick={clear} disabled={!history.length}>
          <Trash2 size={14} />
          Clear history
        </button>
      </div>
      {history.length ? (
        <div className="history-list">
          {history.map((event) => (
            <div className="history-row" key={event.id}>
              <time
                className="mono"
                title={new Date(event.time).toLocaleString()}
              >
                {new Date(event.time).toLocaleTimeString([], { hour12: false })}
              </time>
              <span className={`event-dot ${event.type}`} />
              <div>
                <strong title={event.projectPath}>
                  {event.projectName ?? event.process}
                </strong>{" "}
                {event.type === "started"
                  ? "started listening on"
                  : "stopped listening on"}{" "}
                <span className="mono">:{event.port}</span>
              </div>
              <span className="event-type">
                {event.type === "started" ? "STARTED" : "STOPPED"}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className="empty-state roomy">
          <History size={30} />
          <h3>{enabled ? "A clean slate." : "History is paused."}</h3>
          <p>
            {enabled
              ? "New and stopped listeners will appear here as your workspace changes."
              : "Enable local history in Settings to record port activity."}
          </p>
          <span className="privacy-caption">
            <Shield size={13} />
            Stored on this device only
          </span>
        </div>
      )}
    </section>
  );
}
export function SettingsScreen({
  settings,
  change,
}: {
  settings: Settings;
  change: (s: Settings) => void;
}) {
  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    change({ ...settings, [key]: value });
  }
  return (
    <div className="settings-stack">
      <ProjectPreferences settings={settings} change={change} />
      <section className="panel settings-panel">
        <h3>Monitoring</h3>
        <SettingRow
          title="Refresh interval"
          description="How often Port Authority checks your local sockets."
        >
          <select
            aria-label="Refresh interval"
            value={settings.refreshInterval}
            onChange={(e) => update("refreshInterval", Number(e.target.value))}
          >
            {[1, 2, 5, 10].map((n) => (
              <option key={n} value={n}>
                {n} {n === 1 ? "second" : "seconds"}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title="Show system processes"
          description="Include services owned by the operating system and other users."
        >
          <Toggle
            label="Show system processes"
            checked={settings.showSystem}
            onChange={() => update("showSystem", !settings.showSystem)}
          />
        </SettingRow>
        <SettingRow
          title="Launch at startup"
          description="Not available in this milestone. Open Port Authority when you need it."
        >
          <Toggle
            label="Launch at startup unavailable"
            checked={false}
            onChange={() => {}}
            disabled
          />
        </SettingRow>
      </section>
      <section className="panel settings-panel">
        <h3>Process actions</h3>
        <SettingRow
          title="Confirm before kill"
          description="Ask before sending a graceful termination signal."
        >
          <Toggle
            label="Confirm before kill"
            checked={settings.confirmKill}
            onChange={() => update("confirmKill", !settings.confirmKill)}
          />
        </SettingRow>
        <SettingRow
          title="Confirm before force kill"
          description="Always required. A forced shutdown can discard unsaved work."
        >
          <span className="locked-setting">
            <Shield size={14} />
            Always on
          </span>
        </SettingRow>
        <SettingRow
          title="Default browser protocol"
          description="Used when opening a local service in your browser."
        >
          <select
            aria-label="Default browser protocol"
            value={settings.protocol}
            onChange={(e) =>
              update("protocol", e.target.value as "http" | "https")
            }
          >
            <option value="http">HTTP</option>
            <option value="https">HTTPS</option>
          </select>
        </SettingRow>
      </section>
      <section className="panel settings-panel">
        <h3>History & appearance</h3>
        <SettingRow
          title="Keep local history"
          description="Record started and stopped listeners. Turning off clears saved history."
        >
          <Toggle
            label="Keep local history"
            checked={settings.keepHistory}
            onChange={() => update("keepHistory", !settings.keepHistory)}
          />
        </SettingRow>
        <SettingRow
          title="History retention"
          description="Older events are automatically removed."
        >
          <select
            aria-label="History retention"
            value={settings.retention}
            onChange={(e) => update("retention", Number(e.target.value))}
          >
            {[100, 500, 1000].map((n) => (
              <option key={n} value={n}>
                Last {n} events
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title="Theme"
          description="System currently uses Port Authority’s dark appearance."
        >
          <select
            aria-label="Theme"
            value={settings.theme}
            onChange={(e) =>
              update("theme", e.target.value as "Dark" | "System")
            }
          >
            <option>Dark</option>
            <option>System</option>
          </select>
        </SettingRow>
      </section>
      <p className="settings-privacy">
        <Shield size={15} />
        No analytics. No telemetry. No cloud. Your settings are saved
        automatically on this device.
      </p>
    </div>
  );
}
function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="setting-row">
      <div>
        <strong>{title}</strong>
        <p>{description}</p>
      </div>
      {children}
    </div>
  );
}
export function InsightBanner({ openCheck }: { openCheck: () => void }) {
  return (
    <div className="insight-banner">
      <span className="insight-icon">
        <Eye size={19} />
      </span>
      <div>
        <strong>A clearer view of your localhost.</strong>
        <p>Find the process. Free the port. Get back to building.</p>
      </div>
      <button className="text-button" onClick={openCheck}>
        Check a port
        <ArrowRight size={15} />
      </button>
      <Info className="insight-decoration" size={90} />
    </div>
  );
}
