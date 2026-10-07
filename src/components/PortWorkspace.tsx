import {
  ArrowDownUp,
  Check,
  ChevronDown,
  Copy,
  List,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Star,
  Terminal,
  X,
} from "lucide-react";
import type { Dispatch, MouseEvent, RefObject, SetStateAction } from "react";
import { PortTable } from "./PortTable";
import { Availability, InsightBanner, Toggle } from "./Screens";
import type {
  Filter,
  Page,
  PortEntry,
  ProcessAction,
  Settings,
  SortKey,
} from "../lib/types";
interface Props {
  page: Page;
  ports: PortEntry[];
  favorites: number[];
  filtered: PortEntry[];
  favoriteAvailable: number[];
  availablePort: number | null;
  error: string | null;
  lastScan: Date | null;
  paused: boolean;
  settings: Settings;
  query: string;
  setQuery: (q: string) => void;
  filter: Filter;
  setFilter: (f: Filter) => void;
  filtersOpen: boolean;
  setFiltersOpen: Dispatch<SetStateAction<boolean>>;
  sort: SortKey;
  ascending: boolean;
  onSort: (key: SortKey) => void;
  setAscending: Dispatch<SetStateAction<boolean>>;
  searchRef: RefObject<HTMLInputElement | null>;
  setSettings: Dispatch<SetStateAction<Settings>>;
  navigate: (page: Page) => void;
  toggleFavorite: (port: number) => void;
  copy: (value: string, label: string) => void;
  setSelected: (p: PortEntry) => void;
  open: (p: PortEntry) => void;
  requestAction: (p: PortEntry, action: ProcessAction) => void;
  showMenu: (p: PortEntry, e: MouseEvent) => void;
}
export function PortWorkspace({
  page,
  ports,
  favorites,
  filtered,
  favoriteAvailable,
  availablePort,
  error,
  lastScan,
  paused,
  settings,
  query,
  setQuery,
  filter,
  setFilter,
  filtersOpen,
  setFiltersOpen,
  sort,
  ascending,
  onSort,
  setAscending,
  searchRef,
  setSettings,
  navigate,
  toggleFavorite,
  copy,
  setSelected,
  open,
  requestAction,
  showMenu,
}: Props) {
  return (
    <>
      <section className="ports-panel panel">
        <div className="table-top">
          <div className="view-tabs" role="tablist" aria-label="Port grouping">
            <button
              role="tab"
              aria-selected={page !== "Processes"}
              className={page !== "Processes" ? "active" : ""}
              onClick={() => {
                if (page === "Processes") navigate("Ports");
              }}
            >
              <List size={16} />
              {page === "Favorites" ? "Watched ports" : "Ports view"}
            </button>
            {page !== "Favorites" && (
              <button
                role="tab"
                aria-selected={page === "Processes"}
                className={page === "Processes" ? "active" : ""}
                onClick={() => navigate("Processes")}
              >
                <Terminal size={15} />
                Processes view
              </button>
            )}
          </div>
          <div className="scan-interval">
            <RefreshCw size={12} />
            <span>
              {paused
                ? "Auto-refresh paused"
                : `Updates every ${settings.refreshInterval}s`}
            </span>
          </div>
        </div>
        <div className="table-controls">
          <label className="search-field">
            <Search size={16} />
            <input
              ref={searchRef}
              aria-label="Search ports"
              placeholder="Search projects, ports, or PIDs…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {query ? (
              <button
                className="icon-button"
                title="Clear search"
                onClick={() => setQuery("")}
              >
                <X size={13} />
              </button>
            ) : (
              <kbd>⌘ F</kbd>
            )}
          </label>
          <div className="filter-popover-wrapper">
            <button
              className={`button filter-button ${filtersOpen ? "pressed" : ""}`}
              onClick={() => setFiltersOpen((p) => !p)}
            >
              <SlidersHorizontal size={14} />
              Filters{filter !== "All" && <span className="filter-dot" />}
              <ChevronDown size={12} />
            </button>
            {filtersOpen && (
              <div className="filter-popover">
                <div className="section-label">CONNECTION SCOPE</div>
                {(
                  [
                    "All",
                    "Listening",
                    "Localhost",
                    "External",
                    "System",
                    "User",
                  ] as Filter[]
                ).map((f) => (
                  <button
                    key={f}
                    onClick={() => {
                      setFilter(f);
                      setFiltersOpen(false);
                    }}
                  >
                    <span>{f === "All" ? "All connections" : f}</span>
                    {filter === f && <Check size={14} />}
                  </button>
                ))}
              </div>
            )}
          </div>
          <button
            className="icon-button table-options"
            title="Reverse sort direction"
            onClick={() => setAscending((a) => !a)}
          >
            <ArrowDownUp size={16} />
          </button>
        </div>
        <div className="filter-row">
          <div className="filter-chips">
            {(["All", "TCP", "UDP"] as Filter[]).map((f) => (
              <button
                key={f}
                className={`filter-chip ${filter === f ? "active" : ""}`}
                onClick={() => setFilter(f)}
              >
                {f === "All" ? "All ports" : f}
                <span>
                  {f === "All"
                    ? ports.length
                    : ports.filter((p) => p.protocol === f).length}
                </span>
              </button>
            ))}
            {!["All", "TCP", "UDP"].includes(filter) && (
              <button
                className="filter-chip active"
                onClick={() => setFilter("All")}
              >
                {filter}
                <X size={12} />
              </button>
            )}
          </div>
          <div className="hide-system">
            <span>Hide system processes</span>
            <Toggle
              label="Hide system processes"
              checked={!settings.showSystem}
              onChange={() =>
                setSettings((s) => ({ ...s, showSystem: !s.showSystem }))
              }
            />
          </div>
        </div>
        {availablePort && !error && lastScan ? (
          <Availability
            port={availablePort}
            favorites={favorites}
            toggleFavorite={toggleFavorite}
            copy={copy}
            stale={paused}
          />
        ) : (
          <PortTable
            ports={filtered}
            favorites={favorites}
            toggleFavorite={toggleFavorite}
            select={setSelected}
            open={open}
            kill={(p) => requestAction(p, "kill")}
            restart={(p) => requestAction(p, "restart")}
            menu={showMenu}
            sort={sort}
            ascending={ascending}
            onSort={onSort}
            query={query}
            grouped={page === "Processes"}
          />
        )}
        {page === "Favorites" &&
          !availablePort &&
          !error &&
          favoriteAvailable.map((port) => (
            <div className="favorite-available" key={port}>
              <button
                className="star-button is-favorite"
                title={`Unwatch port ${port}`}
                onClick={() => toggleFavorite(port)}
              >
                <Star size={15} />
              </button>
              <span className="mono">{port}</span>
              <span className="muted">
                {paused ? "Unused at last scan" : "Ready when you are"}
              </span>
              <span className="watch-badge">AVAILABLE</span>
              <button
                className="icon-button"
                title="Copy port"
                onClick={() => {
                  void copy(String(port), "Port");
                }}
              >
                <Copy size={14} />
              </button>
            </div>
          ))}
        <div className="table-footer">
          <span>
            Showing <strong>{availablePort ? 0 : filtered.length}</strong> of{" "}
            <strong>{ports.length}</strong> ports
            {!settings.showSystem && (
              <span className="hidden-count"> · System processes hidden</span>
            )}
          </span>
          <span>
            <span className="table-hint">Click a row to inspect</span>
            <span className="footer-separator">·</span>Right-click for actions
          </span>
        </div>
      </section>
      <InsightBanner openCheck={() => navigate("Check Port")} />
    </>
  );
}
