import {
  ArrowRight,
  CornerDownLeft,
  History,
  RefreshCw,
  Search,
  Settings2,
  Star,
  X,
} from "lucide-react";
import { useState } from "react";
import type { Page, PortEntry } from "../lib/types";
import { filterPorts } from "../lib/ports";
import { Modal } from "./Modal";
export function CommandPalette({
  close,
  navigate,
  refresh,
  ports,
  select,
  search,
}: {
  close: () => void;
  navigate: (page: Page) => void;
  refresh: () => void;
  ports: PortEntry[];
  select: (p: PortEntry) => void;
  search: (value: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const commands = [
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
          name: `${p.port}  ·  ${p.process}`,
          icon: ArrowRight,
          shortcut: p.protocol,
          run: () => select(p),
        }))
    : [];
  const options = [
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
          placeholder="Find a port, process, or command…"
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
            key={c.name}
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
