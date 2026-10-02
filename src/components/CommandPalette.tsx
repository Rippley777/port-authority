import { useProjectContext } from "../features/projects/useProjects";
import { projectSearch } from "../features/projects/types";
import {
  ArrowRight,
  CornerDownLeft,
  History,
  RefreshCw,
  Search,
  Settings2,
  Star,
  Play,
  X,
} from "lucide-react";
import { useState } from "react";
import type { Page, PortEntry } from "../lib/types";
import { filterPorts } from "../lib/ports";
import { Modal } from "./Modal";
import {
  historicalCommandNeedsConfirmation,
  runHistoricalCommand,
} from "../features/recovery/api";
import { useRunHistory } from "../features/recovery/useRunHistory";
export function CommandPalette({
  close,
  navigate,
  refresh,
  ports,
  select,
  search,
  favorites,
  notify,
}: {
  close: () => void;
  navigate: (page: Page) => void;
  refresh: () => void;
  ports: PortEntry[];
  select: (p: PortEntry) => void;
  search: (value: string) => void;
  favorites: number[];
  notify: (message: string, error?: boolean) => void;
}) {
  const projects = useProjectContext();
  const history = useRunHistory({ limit: 100 });
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const commands = [
    { name: "Show projects", icon: Search, run: () => navigate("Projects") },
    {
      name: "Open Conflict Autopilot",
      icon: RefreshCw,
      run: () => navigate("Conflict Autopilot"),
    },
    { name: "Refresh ports", icon: RefreshCw, shortcut: "⌘ R", run: refresh },
    { name: "Show favorites", icon: Star, run: () => navigate("Favorites") },
    {
      name: "Open settings",
      icon: Settings2,
      shortcut: "⌘ ,",
      run: () => navigate("Settings"),
    },
    { name: "Check port", icon: Search, run: () => navigate("Check Port") },
    { name: "View history", icon: History, run: () => navigate("History") },
  ];
  const matched = query
    ? filterPorts(
        ports,
        query.replace(/^find (port|process) /i, ""),
        "All",
        true,
      )
        .slice(0, 5)
        .map((p) => ({
          name: `${p.port}  ·  ${p.project?.name ?? p.process} · ${p.serviceName ?? p.process}`,
          icon: ArrowRight,
          shortcut: p.protocol,
          run: () => select(p),
        }))
    : [];
  const projectCommands = projects.projects
    .flatMap(({ identity: p }) =>
      [
        {
          name: `Open ${p.name} in ${projects.editorName}`,
          icon: ArrowRight,
          run: () => void projects.action(p, "editor"),
        },
        {
          name: `Open ${p.name} terminal`,
          icon: ArrowRight,
          run: () => void projects.action(p, "terminal"),
        },
        ...(p.repository?.webUrl
          ? [
              {
                name: `Open ${p.name} repository`,
                icon: ArrowRight,
                run: () => void projects.action(p, "repository"),
              },
            ]
          : []),
        {
          name: `Reveal ${p.name}`,
          icon: ArrowRight,
          run: () => void projects.action(p, "reveal"),
        },
        {
          name: `Show ${p.name} ports`,
          icon: Search,
          run: () => projects.showPorts(p),
        },
      ]
        .filter(
          (command) =>
            query &&
            (command.name.toLowerCase().includes(query.toLowerCase()) ||
              projectSearch(p).includes(query.toLowerCase())),
        )
        .map((command) => ({ ...command, shortcut: p.displayPath })),
    )
    .slice(0, 40);
  const historicalCommands = (history.data?.commands ?? [])
    .filter(
      (command) =>
        command.launchContext.recoverable &&
        !historicalCommandNeedsConfirmation(command.launchContext),
    )
    .filter((command) => {
      const value =
        `${command.latestRun.projectName ?? ""} ${command.launchContext.command} ${command.launchContext.workingDirectory}`.toLowerCase();
      return (
        query &&
        query
          .toLowerCase()
          .split(/\s+/)
          .filter(Boolean)
          .every((term) => value.includes(term))
      );
    })
    .sort((a, b) => {
      const aPriority = a.pinnedPorts.some((port) => favorites.includes(port))
        ? 2
        : a.typicalPorts.some((port) => favorites.includes(port))
          ? 1
          : 0;
      const bPriority = b.pinnedPorts.some((port) => favorites.includes(port))
        ? 2
        : b.typicalPorts.some((port) => favorites.includes(port))
          ? 1
          : 0;
      return (
        bPriority - aPriority || b.latestRun.startedAt - a.latestRun.startedAt
      );
    })
    .slice(0, 12)
    .map((command) => {
      const active = command.active
        ? ports.find(
            (entry) =>
              entry.launch?.id === command.launchContext.id ||
              entry.project?.rootPath === command.launchContext.projectId,
          )
        : undefined;
      return {
        name: `${active ? "Open" : "Run"} ${command.latestRun.projectName ?? command.latestRun.processName ?? "command"} — ${command.launchContext.command}`,
        icon: active ? ArrowRight : Play,
        shortcut: command.typicalPorts.map((port) => `:${port}`).join(" "),
        run: () => {
          if (active) select(active);
          else
            void runHistoricalCommand(command.launchContext.id)
              .then((result) => notify(result.message))
              .catch((reason) => notify(String(reason), true));
        },
      };
    });
  const options: {
    name: string;
    icon: typeof Search;
    shortcut?: string;
    run: () => void;
  }[] = [
    ...historicalCommands,
    ...projectCommands,
    ...matched,
    ...commands.filter((c) =>
      c.name.toLowerCase().includes(query.toLowerCase()),
    ),
  ];
  if (query)
    options.push({
      name: `Search ports for “${query}”`,
      icon: Search,
      shortcut: "↵",
      run: () => search(query),
    });
  function run(index: number) {
    options[index]?.run();
    close();
  }
  return (
    <Modal close={close} label="Command palette" className="palette-backdrop">
      <div className="palette-input">
        <Search size={20} />
        <input
          aria-label="Search commands and ports"
          placeholder="Find a project, port, or command…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((i) => Math.min(i + 1, options.length - 1));
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((i) => Math.max(i - 1, 0));
            }
            if (e.key === "Enter") run(active);
          }}
        />
        <button
          className="icon-button"
          onClick={close}
          aria-label="Close command palette"
        >
          <X size={16} />
        </button>
      </div>
      <div className="palette-results">
        <div className="section-label">
          {query ? "RESULTS" : "QUICK ACTIONS"}
        </div>
        {options.map((c, i) => (
          <button
            key={`${c.name}:${c.shortcut ?? ""}`}
            className={`palette-option ${i === active ? "selected" : ""}`}
            onMouseEnter={() => setActive(i)}
            onClick={() => run(i)}
          >
            <c.icon size={17} />
            <span>{c.name}</span>
            {c.shortcut && <kbd>{c.shortcut}</kbd>}
          </button>
        ))}
      </div>
      <div className="palette-footer">
        <span>
          <kbd>↑</kbd>
          <kbd>↓</kbd> to navigate
        </span>
        <span>
          <CornerDownLeft size={12} /> to select
        </span>
        <span>
          <kbd>esc</kbd> to close
        </span>
      </div>
    </Modal>
  );
}
