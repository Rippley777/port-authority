import { ProjectActions } from "../features/projects/ProjectActions";
import {
  AlertCircle,
  Copy,
  ExternalLink,
  FolderOpen,
  RotateCw,
  Square,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { desktop } from "../lib/api";
import { connectionAddress } from "../lib/ports";
import type { PortEntry, ProcessAction } from "../lib/types";
export interface PortMenu {
  entry: PortEntry;
  x: number;
  y: number;
}
interface Props {
  menu: PortMenu;
  setMenu: (value: PortMenu | null) => void;
  open: (p: PortEntry) => void;
  copy: (s: string, l: string) => Promise<void>;
  reveal: (p: PortEntry) => Promise<void>;
  requestAction: (p: PortEntry, a: ProcessAction) => void;
  inspect: (p: PortEntry) => void;
}
export function PortContextMenu({
  menu,
  setMenu,
  open,
  copy,
  reveal,
  requestAction,
  inspect,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) {
      const bounds = el.getBoundingClientRect();
      el.style.top = `${Math.max(8, Math.min(menu.y, window.innerHeight - bounds.height - 8))}px`;
      el.style.left = `${Math.max(8, Math.min(menu.x, window.innerWidth - bounds.width - 8))}px`;
    }
    ref.current
      ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
      ?.focus();
  }, []);
  return (
    <div
      ref={ref}
      role="menu"
      onKeyDown={(e) => {
        const buttons = [
          ...(ref.current?.querySelectorAll<HTMLButtonElement>(
            "button:not(:disabled)",
          ) ?? []),
        ];
        const index = buttons.indexOf(
          document.activeElement as HTMLButtonElement,
        );
        if (e.key === "ArrowDown") {
          e.preventDefault();
          buttons[(index + 1) % buttons.length]?.focus();
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          buttons[(index - 1 + buttons.length) % buttons.length]?.focus();
        }
        if (e.key === "Tab" || e.key === "Escape") setMenu(null);
      }}
      className="context-menu"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="context-heading">
        <span className="mono">:{menu.entry.port}</span>
        <span>{menu.entry.project?.name ?? menu.entry.process}</span>
      </div>
      {menu.entry.project && (
        <>
          <ProjectActions
            project={menu.entry.project}
            menu
            done={() => setMenu(null)}
          />
          <div className="menu-separator" />
        </>
      )}
      <MenuItem
        icon={AlertCircle}
        label="Inspect Process"
        onClick={() => {
          inspect(menu.entry);
          setMenu(null);
        }}
      />
      <MenuItem
        icon={ExternalLink}
        label="Open in browser"
        onClick={() => {
          open(menu.entry);
          setMenu(null);
        }}
      />
      {[
        ["Copy address", connectionAddress(menu.entry)],
        ["Copy port", String(menu.entry.port)],
        ["Copy PID", menu.entry.pid === null ? "" : String(menu.entry.pid)],
        ["Copy command", menu.entry.command.join(" ")],
        ["Copy process path", menu.entry.executable ?? ""],
      ].map(([label, value]) => (
        <MenuItem
          key={label}
          icon={Copy}
          label={label}
          disabled={!value}
          onClick={() => {
            void copy(value, label.replace("Copy ", ""));
            setMenu(null);
          }}
        />
      ))}
      <div className="menu-separator" />
      <MenuItem
        icon={FolderOpen}
        label="Reveal executable"
        disabled={!menu.entry.executable || !desktop}
        onClick={() => {
          void reveal(menu.entry);
          setMenu(null);
        }}
      />
      <MenuItem
        icon={RotateCw}
        label="Restart"
        disabled={!menu.entry.restartable}
        title={menu.entry.restartReason}
        onClick={() => {
          requestAction(menu.entry, "restart");
          setMenu(null);
        }}
      />
      <div className="menu-separator" />
      <MenuItem
        icon={Square}
        label="Kill process"
        disabled={menu.entry.protected || !menu.entry.pid}
        danger
        onClick={() => {
          requestAction(menu.entry, "kill");
          setMenu(null);
        }}
      />
      <MenuItem
        icon={AlertCircle}
        label="Force kill…"
        disabled={menu.entry.protected || !menu.entry.pid}
        danger
        onClick={() => {
          requestAction(menu.entry, "force");
          setMenu(null);
        }}
      />
    </div>
  );
}
function MenuItem({
  icon: Icon,
  label,
  onClick,
  disabled,
  danger,
  title,
}: {
  icon: typeof Copy;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
  title?: string;
}) {
  return (
    <button
      role="menuitem"
      className={danger ? "danger-text" : ""}
      onClick={onClick}
      disabled={disabled}
      title={title}
    >
      <Icon size={14} />
      <span>{label}</span>
    </button>
  );
}
