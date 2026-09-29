import { parseSettings, parseFavorites } from "../lib/persistence";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent } from "react";
import { usePersistentState } from "./usePersistentState";
import { usePorts } from "./usePorts";
import { filterPorts, isHttp, portFromQuery, sortPorts } from "../lib/ports";
import {
  writeClipboard,
  controlProcess,
  desktop,
  openPort,
  revealExecutable,
} from "../lib/api";
import { defaultSettings } from "../lib/types";
import type {
  Filter,
  Page,
  PortEntry,
  ProcessAction,
  Settings,
  SortKey,
} from "../lib/types";
import type { PortMenu } from "../components/PortContextMenu";
export function useWorkspace() {
  const [page, setPage] = useState<Page>("Ports");
  const [settings, setSettings] = usePersistentState<Settings>(
    "pa-settings",
    defaultSettings,
    parseSettings,
  );
  const [favorites, setFavorites] = usePersistentState<number[]>(
    "pa-favorites",
    [3000, 5173, 5432],
    parseFavorites,
  );
  const monitor = usePorts(settings);
  const {
    ports,
    paused,
    setPaused,
    refresh,
    scanning,
    error,
    history,
    lastScan,
  } = monitor;
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("All");
  const [sort, setSort] = useState<SortKey>("port");
  const [ascending, setAscending] = useState(true);
  const [selected, setSelected] = useState<PortEntry | null>(null);
  const [palette, setPalette] = useState(false);
  const [help, setHelp] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [menu, setMenu] = useState<PortMenu | null>(null);
  const [confirmation, setConfirmation] = useState<{
    entry: PortEntry;
    action: ProcessAction;
  } | null>(null);
  const [manualOpen, setManualOpen] = useState<PortEntry | null>(null);
  const [acting, setActing] = useState(false);
  const [toast, setToast] = useState<{
    message: string;
    error?: boolean;
  } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const closeDrawer = useCallback(() => setSelected(null), []);
  const closePalette = useCallback(() => setPalette(false), []);
  const closeHelp = useCallback(() => setHelp(false), []);
  const closeConfirmation = useCallback(() => {
    if (!acting) setConfirmation(null);
  }, [acting]);
  const closeManual = useCallback(() => setManualOpen(null), []);
  const navigate = useCallback((next: Page) => {
    setPage(next);
    setQuery("");
    setFilter("All");
    setSelected(null);
    setFiltersOpen(false);
  }, []);
  const notify = useCallback(
    (message: string, isError = false) => setToast({ message, error: isError }),
    [],
  );
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), toast.error ? 7000 : 3200);
    return () => clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    function key(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((p) => !p);
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "r") {
        e.preventDefault();
        void refresh();
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        navigate("Ports");
        requestAnimationFrame(() => searchRef.current?.focus());
      }
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        navigate("Settings");
      }
      if (e.key === "Escape") {
        setMenu(null);
        setFiltersOpen(false);
      }
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [navigate, refresh]);
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    document.addEventListener("click", close);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("click", close);
      window.removeEventListener("resize", close);
    };
  }, [menu]);
  function toggleFavorite(port: number) {
    setFavorites((f) =>
      f.includes(port)
        ? f.filter((p) => p !== port)
        : [...f, port].sort((a, b) => a - b),
    );
  }
  async function copy(text: string, label: string) {
    try {
      await writeClipboard(text);
      notify(`${label} copied to clipboard`);
    } catch {
      notify(
        "Clipboard access was denied. Check this app’s clipboard permissions.",
        true,
      );
    }
  }
  async function doOpen(entry: PortEntry) {
    try {
      await openPort(entry, settings.protocol);
    } catch (e) {
      notify(String(e), true);
    }
  }
  function open(entry: PortEntry) {
    if (isHttp(entry)) void doOpen(entry);
    else setManualOpen(entry);
  }
  async function perform(entry: PortEntry, action: ProcessAction) {
    setActing(true);
    try {
      await controlProcess(entry, action);
      setSelected(null);
      setConfirmation(null);
      await refresh();
      notify(
        desktop
          ? `${action === "force" ? "Force kill" : "Termination"} signal sent to ${entry.process} (${entry.pid})`
          : `Preview: ${entry.process} stopped. No real process was affected.`,
      );
    } catch (e) {
      notify(String(e), true);
    } finally {
      setActing(false);
    }
  }
  function requestAction(entry: PortEntry, action: ProcessAction) {
    if (entry.protected || !entry.pid) {
      notify("This process is protected. Process control is disabled.", true);
      return;
    }
    if (
      action === "force" ||
      action === "restart" ||
      entry.system ||
      settings.confirmKill
    )
      setConfirmation({ entry, action });
    else void perform(entry, action);
  }
  async function reveal(entry: PortEntry) {
    try {
      await revealExecutable(entry);
    } catch (e) {
      notify(String(e), true);
    }
  }
  function showMenu(entry: PortEntry, event: MouseEvent) {
    event.stopPropagation();
    setMenu({
      entry,
      x: Math.max(12, Math.min(event.clientX, window.innerWidth - 245)),
      y: Math.max(12, Math.min(event.clientY, window.innerHeight - 452)),
    });
  }
  function onSort(key: SortKey) {
    if (sort === key) setAscending((a) => !a);
    else {
      setSort(key);
      setAscending(key !== "startedAt");
    }
  }
  const filtered = useMemo(
    () =>
      sortPorts(
        filterPorts(
          page === "Favorites"
            ? ports.filter((p) => favorites.includes(p.port))
            : ports,
          query,
          filter,
          settings.showSystem,
        ),
        sort,
        ascending,
      ),
    [
      ports,
      page,
      favorites,
      query,
      filter,
      settings.showSystem,
      sort,
      ascending,
    ],
  );
  const queriedPort = portFromQuery(query);
  const availablePort =
    queriedPort && !ports.some((p) => p.port === queriedPort)
      ? queriedPort
      : null;
  const favoriteAvailable = favorites.filter(
    (port) =>
      !ports.some((p) => p.port === port) &&
      (!query || String(port).includes(query.replace(/^:/, ""))),
  );
  const isTablePage = ["Ports", "Processes", "Favorites"].includes(page);
  const selectedEntry = selected
    ? (ports.find((p) => p.id === selected.id) ?? selected)
    : null;
  const scanningStatus = error
    ? "Scan unavailable"
    : paused
      ? "Updates paused"
      : "Live";
  return {
    scanningStatus,
    page,
    settings,
    setSettings,
    favorites,
    monitor,
    ports,
    paused,
    setPaused,
    refresh,
    scanning,
    error,
    history,
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
  };
}
