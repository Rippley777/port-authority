import { trackView } from "./lib/analytics";
import { useEffect, useState } from "react";
import { TimelineNavigation } from "./features/timeline/useTimeline";
import { HistoryPage } from "./features/recovery/HistoryPage";
import { FavoritesPage } from "./features/recovery/FavoritesPage";
import { ProjectsContext, useProjects } from "./features/projects/useProjects";
import {
  ProjectsScreen,
  ProjectDetails,
} from "./features/projects/ProjectsScreen";
import { useAutopilot } from "./hooks/useAutopilot";
import {
  AutopilotPage,
  ConflictBanner,
} from "./components/autopilot/AutopilotPage";
import { PortContextMenu } from "./components/PortContextMenu";
import { OpenServiceDialog } from "./components/OpenServiceDialog";
import { ConfirmProcessDialog } from "./components/ConfirmProcessDialog";
import { HelpDialog } from "./components/HelpDialog";
import {
  AlertCircle,
  Check,
  ChevronRight,
  Pause,
  Play,
  Plus,
  RefreshCw,
  ShieldCheck,
  X,
} from "lucide-react";
import { Sidebar, AppHeader } from "./components/Sidebar";
import { PortWorkspace } from "./components/PortWorkspace";
import { ProcessDrawer } from "./components/ProcessDrawer";
import { CommandPalette } from "./components/CommandPalette";
import {
  CheckPort,
  Overview,
  SettingsScreen,
  Stats,
} from "./components/Screens";
import { useWorkspace } from "./hooks/useWorkspace";
import { desktop } from "./lib/api";
import type { Page } from "./lib/types";
const descriptions: Record<Page, string> = {
  "Conflict Autopilot": "Port conflicts, with a clear next step.",
  Projects: "Your projects. A home for every local service.",
  Ports: "Your local ports. A little less mystery.",
  Processes: "The processes behind your local services.",
  Overview: "A little clarity for your local environment.",
  Favorites: "The ports you care about—and the commands that bring them back.",
  History: "What you ran, where it ran, and how to run it again.",
  "Check Port": "Find a free port. Or find what’s in the way.",
  Settings: "Make yourself at home.",
};
export default function App() {
  const {
    scanningStatus,
    page,
    settings,
    setSettings,
    favorites,
    ports,
    paused,
    setPaused,
    refresh,
    scanning,
    error,
    lastScan,
    query,
    setQuery,
    filter,
    setFilter,
    sort,
    ascending,
    setAscending,
    selectedEntry,
    setSelected,
    palette,
    setPalette,
    help,
    setHelp,
    filtersOpen,
    setFiltersOpen,
    menu,
    setMenu,
    confirmation,
    manualOpen,
    acting,
    toast,
    setToast,
    searchRef,
    closeDrawer,
    closePalette,
    closeHelp,
    closeConfirmation,
    closeManual,
    navigate,
    toggleFavorite,
    copy,
    doOpen,
    open,
    perform,
    requestAction,
    reveal,
    showMenu,
    onSort,
    filtered,
    availablePort,
    favoriteAvailable,
    isTablePage,
  } = useWorkspace();
  useEffect(() => {
    trackView(page);
  }, [page]);
  const [timelinePort, setTimelinePort] = useState<number | undefined>();
  const openTimeline = (port?: number) => {
    setTimelinePort(port);
    setSelected(null);
    navigate("History");
  };
  const autopilot = useAutopilot(refresh);
  const projects = useProjects(
    ports,
    settings,
    (message, error) => setToast({ message, error }),
    refresh,
    (p) => {
      navigate("Ports");
      setQuery(p.rootPath);
    },
  );
  return (
    <TimelineNavigation.Provider value={openTimeline}>
      <ProjectsContext.Provider value={projects}>
        <div className="app-shell">
          <Sidebar
            page={page}
            navigate={(page) => {
              if (page === "History") setTimelinePort(undefined);
              navigate(page);
            }}
            count={ports.length}
            conflicts={
              autopilot.snapshot?.conflicts.filter((c) =>
                ["detected", "force_required", "failed"].includes(c.status),
              ).length ?? 0
            }
            favorites={favorites.length}
            paused={paused}
            failed={!!error}
            toggleLive={() => setPaused((p) => !p)}
            openPalette={() => setPalette(true)}
            openHelp={() => setHelp(true)}
          />
          <div className="main-shell">
            <AppHeader page={page} openPalette={() => setPalette(true)} />
            <main className="main-content">
              <div className="page-heading">
                <div>
                  <div className="title-line">
                    <h1>{page}</h1>
                    {page === "Ports" && (
                      <span className="heading-count">{ports.length}</span>
                    )}
                    {page === "Favorites" && (
                      <span className="heading-count">{favorites.length}</span>
                    )}
                  </div>
                  <p>{descriptions[page]}</p>
                </div>
                <div className="heading-actions">
                  {page !== "Settings" &&
                    page !== "History" &&
                    page !== "Conflict Autopilot" && (
                      <>
                        <button
                          className={`live-control ${paused || error ? "is-paused" : ""}`}
                          onClick={() => setPaused((p) => !p)}
                          aria-label={
                            paused
                              ? "Resume live updates"
                              : "Pause live updates"
                          }
                          title={
                            paused
                              ? "Resume live updates"
                              : "Pause live updates"
                          }
                        >
                          <span className="status-dot" />
                          {scanningStatus}
                          <span className="live-divider" />
                          {paused ? <Play size={12} /> : <Pause size={12} />}
                        </button>
                        <button
                          className="button refresh-button"
                          onClick={() => {
                            void refresh();
                          }}
                          disabled={scanning}
                          title="Refresh ports (⌘R)"
                        >
                          <RefreshCw
                            className={scanning ? "spin" : ""}
                            size={14}
                          />
                          Refresh
                        </button>
                      </>
                    )}
                  {page === "Favorites" && (
                    <button
                      className="button primary"
                      onClick={() => navigate("Check Port")}
                    >
                      <Plus size={14} />
                      Watch a port
                    </button>
                  )}
                </div>
              </div>
              {page !== "Conflict Autopilot" && (
                <ConflictBanner
                  controller={autopilot}
                  open={() => navigate("Conflict Autopilot")}
                />
              )}
              {page === "Conflict Autopilot" && (
                <AutopilotPage
                  controller={autopilot}
                  inspect={setSelected}
                  copy={copy}
                />
              )}
              {error && (
                <div className="error-banner" role="alert">
                  <AlertCircle size={18} />
                  <div>
                    <strong>Unable to refresh ports</strong>
                    <p>{error}</p>
                  </div>
                  <button
                    className="button"
                    onClick={() => {
                      void refresh();
                    }}
                  >
                    Try again
                  </button>
                </div>
              )}
              {(page === "Ports" ||
                page === "Overview" ||
                page === "Processes") && <Stats ports={ports} />}
              {isTablePage && page !== "Favorites" && (
                <PortWorkspace
                  {...{
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
                  }}
                />
              )}
              {page === "Favorites" && (
                <FavoritesPage
                  favorites={favorites}
                  ports={ports}
                  paused={paused}
                  toggleFavorite={toggleFavorite}
                  inspect={setSelected}
                  requestAction={requestAction}
                  openTimeline={openTimeline}
                  refreshPorts={refresh}
                  notify={(message, isError) =>
                    setToast({ message, error: isError })
                  }
                />
              )}
              {page === "Projects" && <ProjectsScreen inspect={setSelected} />}
              {page === "Overview" && (
                <Overview
                  ports={ports}
                  favorites={favorites}
                  select={setSelected}
                  navigateFavorites={() => navigate("Favorites")}
                />
              )}
              {page === "Check Port" &&
                (error || !lastScan ? (
                  <div className="panel empty-state">
                    <AlertCircle size={28} />
                    <h3>Waiting for a successful scan</h3>
                    <p>
                      Port availability will appear when the local socket scan
                      completes.
                    </p>
                  </div>
                ) : (
                  <CheckPort
                    ports={ports}
                    favorites={favorites}
                    toggleFavorite={toggleFavorite}
                    copy={copy}
                    select={setSelected}
                    stale={paused}
                  />
                ))}
              {page === "History" && (
                <HistoryPage
                  initialPort={timelinePort}
                  ports={ports}
                  favorites={favorites}
                  paused={paused}
                  failed={!!error || !lastScan}
                  inspect={setSelected}
                  toggleFavorite={toggleFavorite}
                  copy={copy}
                  refreshPorts={refresh}
                  notify={(message, isError) =>
                    setToast({ message, error: isError })
                  }
                />
              )}
              {page === "Settings" && (
                <SettingsScreen settings={settings} change={setSettings} />
              )}
              <div className="workspace-footnote">
                <ShieldCheck size={13} />
                <span>
                  {desktop
                    ? "Everything happens on your machine."
                    : "Interactive preview · Sample data. Desktop app monitors your real ports."}
                </span>
                <button onClick={() => setHelp(true)}>
                  Learn more
                  <ChevronRight size={12} />
                </button>
              </div>
            </main>
            <footer className="status-bar">
              <span>
                <span
                  className={`status-dot ${paused || error ? "paused" : ""}`}
                />
                {error ? "Scan error" : paused ? "Paused" : "All systems local"}
                <span className="status-bar-divider" />
                {ports.length} active ports
              </span>
              <span>
                {lastScan
                  ? `Last scan ${lastScan.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false })}`
                  : "Waiting for first scan"}
                <span className="status-bar-divider" />
                <button onClick={() => setPalette(true)}>
                  <kbd>⌘ K</kbd>Command palette
                </button>
              </span>
            </footer>
          </div>
          <ProjectDetails inspect={setSelected} />
          {selectedEntry && !projects.selected && (
            <ProcessDrawer
              entry={selectedEntry}
              close={closeDrawer}
              favorite={favorites.includes(selectedEntry.port)}
              toggleFavorite={() => toggleFavorite(selectedEntry.port)}
              copy={copy}
              open={() => open(selectedEntry)}
              control={(action) => requestAction(selectedEntry, action)}
              reveal={() => {
                void reveal(selectedEntry);
              }}
            />
          )}
          {palette && (
            <CommandPalette
              close={closePalette}
              navigate={navigate}
              refresh={() => {
                void refresh();
              }}
              ports={ports}
              select={setSelected}
              search={(value) => {
                navigate("Ports");
                setQuery(value.replace(/^find (port|process) /i, ""));
              }}
              favorites={favorites}
              notify={(message, isError) =>
                setToast({ message, error: isError })
              }
            />
          )}
          {confirmation && (
            <ConfirmProcessDialog
              {...{ confirmation, closeConfirmation, acting, ports, perform }}
            />
          )}
          {manualOpen && (
            <OpenServiceDialog
              {...{ manualOpen, closeManual, settings, doOpen }}
            />
          )}
          {help && <HelpDialog closeHelp={closeHelp} />}
          {menu && (
            <PortContextMenu
              {...{ menu, setMenu, open, copy, reveal, requestAction }}
              inspect={setSelected}
            />
          )}
          {toast && (
            <div
              className={`toast ${toast.error ? "toast-error" : ""}`}
              role={toast.error ? "alert" : "status"}
            >
              {toast.error ? <AlertCircle size={17} /> : <Check size={17} />}
              <span>{toast.message}</span>
              <button
                className="icon-button"
                onClick={() => setToast(null)}
                aria-label="Dismiss notification"
              >
                <X size={14} />
              </button>
            </div>
          )}
        </div>
      </ProjectsContext.Provider>
    </TimelineNavigation.Provider>
  );
}
