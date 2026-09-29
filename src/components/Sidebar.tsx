import {
  Anchor,
  ArrowUpRight,
  CircleHelp,
  Command,
  History,
  LayoutDashboard,
  Network,
  Radar,
  Search,
  Settings2,
  ShieldCheck,
  Star,
  Terminal,
  Waves,
} from "lucide-react";
import type { Page } from "../lib/types";
import { desktop } from "../lib/api";
interface Props {
  page: Page;
  navigate: (page: Page) => void;
  count: number;
  conflicts: number;
  favorites: number;
  paused: boolean;
  failed: boolean;
  toggleLive: () => void;
  openPalette: () => void;
  openHelp: () => void;
}
export function Sidebar({
  page,
  navigate,
  count,
  conflicts,
  favorites,
  paused,
  failed,
  toggleLive,
  openPalette,
  openHelp,
}: Props) {
  const main = [
    { name: "Overview", icon: LayoutDashboard },
    { name: "Ports", icon: Network },
    { name: "Processes", icon: Terminal },
    { name: "Favorites", icon: Star },
    { name: "History", icon: History },
    { name: "Conflict Autopilot", icon: Radar },
  ] as const;
  return (
    <aside className="sidebar">
      <a
        className="brand"
        href="#"
        onClick={(e) => {
          e.preventDefault();
          navigate("Ports");
        }}
      >
        <span className="brand-mark">
          <Anchor size={24} strokeWidth={1.8} />
        </span>
        <span>
          Port Authority<small>KNOW WHAT’S LISTENING.</small>
        </span>
      </a>
      <button className="sidebar-search" onClick={openPalette}>
        <Search size={15} />
        <span>Quick search</span>
        <kbd>⌘ K</kbd>
      </button>
      <div className="nav-label">WORKSPACE</div>
      <nav aria-label="Main navigation">
        {main.map(({ name, icon: Icon }) => (
          <button
            key={name}
            className={`nav-item ${page === name ? "active" : ""}`}
            onClick={() => navigate(name)}
          >
            <Icon size={18} strokeWidth={1.7} />
            <span>{name === "Conflict Autopilot" ? "Autopilot" : name}</span>
            {name === "Conflict Autopilot" && conflicts > 0 && (
              <span className="nav-count conflict-count">{conflicts}</span>
            )}
            {name === "Ports" && <span className="nav-count">{count}</span>}
            {name === "Favorites" && favorites > 0 && (
              <span className="nav-count">{favorites}</span>
            )}
          </button>
        ))}
      </nav>
      <div className="nav-label tools-label">TOOLS</div>
      <nav aria-label="Tools">
        <button
          className={`nav-item ${page === "Check Port" ? "active" : ""}`}
          onClick={() => navigate("Check Port")}
        >
          <Search size={18} strokeWidth={1.7} />
          <span>Check a port</span>
          <span className="small-arrow">↗</span>
        </button>
        <button
          className={`nav-item ${page === "Settings" ? "active" : ""}`}
          onClick={() => navigate("Settings")}
        >
          <Settings2 size={18} strokeWidth={1.7} />
          <span>Settings</span>
        </button>
      </nav>
      <div className="sidebar-bottom">
        <div className="local-note">
          <span className="local-note-icon">
            <ShieldCheck size={20} />
          </span>
          <strong>Your machine. Your data.</strong>
          <p>
            Entirely local. Always private.
            <br />
            No accounts, no telemetry.
          </p>
        </div>
        <button className="help-link" onClick={openHelp}>
          <CircleHelp size={16} />
          <span>Help & shortcuts</span>
          <ArrowUpRight size={14} />
        </button>
        <div className="sidebar-status">
          <button onClick={toggleLive}>
            <span
              className={`status-dot ${paused || failed ? "paused" : ""}`}
            />
            {failed
              ? "Scan unavailable"
              : paused
                ? "Monitoring paused"
                : "Live monitoring"}
          </button>
          <span>v0.1.0</span>
        </div>
      </div>
    </aside>
  );
}
export function AppHeader({
  page,
  openPalette,
}: {
  page: Page;
  openPalette: () => void;
}) {
  return (
    <header className="app-header">
      <div className="breadcrumb">
        <Waves size={17} />
        <span>Workspace</span>
        <span className="breadcrumb-slash">/</span>
        <strong>{page}</strong>
      </div>
      <div className="header-right">
        <span className="environment">
          <span className="tiny-dot" />
          {desktop ? "LOCAL MACHINE" : "PREVIEW WORKSPACE"}
        </span>
        <button
          className="icon-button command-button"
          title="Command palette (⌘K)"
          onClick={openPalette}
        >
          <Command size={16} />
        </button>
      </div>
    </header>
  );
}
