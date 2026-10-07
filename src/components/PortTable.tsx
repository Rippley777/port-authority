import { ProjectBadge } from "../features/projects/ProjectBadge";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ExternalLink,
  MoreHorizontal,
  Network,
  RotateCw,
  SearchX,
  Shield,
  Square,
  Star,
} from "lucide-react";
import type { MouseEvent } from "react";
import { displayAddress, isHttp, uptime } from "../lib/ports";
import type { PortEntry, SortKey } from "../lib/types";
import { ServiceIcon } from "./ServiceIcon";
interface Props {
  ports: PortEntry[];
  favorites: number[];
  toggleFavorite: (port: number) => void;
  select: (p: PortEntry) => void;
  open: (p: PortEntry) => void;
  kill: (p: PortEntry) => void;
  restart: (p: PortEntry) => void;
  menu: (p: PortEntry, event: MouseEvent) => void;
  sort: SortKey;
  ascending: boolean;
  onSort: (key: SortKey) => void;
  query: string;
  grouped?: boolean;
}
export function PortTable(props: Props) {
  const {
    ports,
    favorites,
    toggleFavorite,
    select,
    open,
    kill,
    restart,
    menu,
    sort,
    ascending,
    onSort,
    query,
    grouped,
  } = props;
  const headers: { label: string; key?: SortKey }[] = [
    { label: "PORT", key: "port" },
    { label: "PROJECT / SERVICE", key: "process" },
    { label: "PID", key: "pid" },
    { label: "ADDRESS" },
    { label: "PROTOCOL", key: "protocol" },
    { label: "STATUS" },
    { label: "UPTIME", key: "startedAt" },
    { label: "ACTIONS" },
  ];
  const row = (p: PortEntry) => (
    <tr
      key={p.id}
      onClick={() => select(p)}
      onContextMenu={(e) => {
        e.preventDefault();
        menu(p, e);
      }}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" && e.target === e.currentTarget) select(p);
      }}
      aria-label={`Inspect ${p.process} on port ${p.port}`}
    >
      <td className="star-cell">
        <button
          className={`star-button ${favorites.includes(p.port) ? "is-favorite" : ""}`}
          title={`${favorites.includes(p.port) ? "Unwatch" : "Watch"} port ${p.port}`}
          onClick={(e) => {
            e.stopPropagation();
            toggleFavorite(p.port);
          }}
        >
          <Star size={15} />
        </button>
      </td>
      <td>
        <span className="port-number">{p.port}</span>
      </td>
      <td>
        <div className="process-cell">
          <ServiceIcon entry={p} />
          <ProjectBadge entry={p} />
          {p.system && (
            <Shield
              size={12}
              className="system-icon"
              aria-label="System process"
            />
          )}
        </div>
      </td>
      <td className="muted mono">{p.pid ?? "—"}</td>
      <td className="address-cell" title={p.address}>
        {displayAddress(p.address)}
      </td>
      <td>
        <span className={`protocol ${p.protocol.toLowerCase()}`}>
          {p.protocol}
        </span>
      </td>

      <td>
        <span className={`listening ${p.protocol === "UDP" ? "bound" : ""}`}>
          <span className="status-dot" />
          {p.protocol === "TCP" ? "Listening" : "Bound"}
        </span>
      </td>
      <td className="muted mono uptime-cell">{uptime(p.startedAt)}</td>
      <td>
        <div className="row-actions">
          <button
            className={`icon-button ${!isHttp(p) ? "subdued" : ""}`}
            title={
              isHttp(p)
                ? `Open localhost:${p.port}`
                : "Open as HTTP (service may not support HTTP)"
            }
            onClick={(e) => {
              e.stopPropagation();
              open(p);
            }}
          >
            <ExternalLink size={14} />
          </button>
          <button
            className="icon-button kill-icon"
            title={
              p.protected
                ? "Protected system process"
                : `Terminate ${p.process}`
            }
            disabled={p.protected || !p.pid}
            onClick={(e) => {
              e.stopPropagation();
              kill(p);
            }}
          >
            <Square size={12} />
          </button>
          <button
            className="icon-button"
            aria-label={`Restart service on port ${p.port}`}
            title={p.restartable ? `Restart on :${p.port}` : p.restartReason}
            disabled={!p.restartable || p.protected || !p.pid}
            onClick={(e) => {
              e.stopPropagation();
              restart(p);
            }}
          >
            <RotateCw size={14} />
          </button>
          <button
            className="icon-button"
            title={`More actions for port ${p.port}`}
            onClick={(e) => {
              e.stopPropagation();
              menu(p, e);
            }}
          >
            <MoreHorizontal size={18} />
          </button>
        </div>
      </td>
    </tr>
  );
  const groups = new Map<string, PortEntry[]>();
  if (grouped)
    for (const p of ports) {
      const key = p.pid === null ? p.id : `${p.pid}`;
      groups.set(key, [...(groups.get(key) ?? []), p]);
    }
  return (
    <div className="table-scroll">
      <table className="port-table">
        <colgroup>
          <col className="col-star" />
          <col className="col-port" />
          <col className="col-process" />
          <col className="col-pid" />
          <col className="col-address" />
          <col className="col-protocol" />
          <col className="col-status" />
          <col className="col-uptime" />
          <col className="col-actions" />
        </colgroup>
        <thead>
          <tr>
            <th />
            <>
              {headers.map((h) => (
                <th
                  key={h.label}
                  aria-sort={
                    h.key && sort === h.key
                      ? ascending
                        ? "ascending"
                        : "descending"
                      : undefined
                  }
                >
                  {h.key ? (
                    <button onClick={() => onSort(h.key!)}>
                      {h.label}
                      {sort === h.key ? (
                        ascending ? (
                          <ArrowUp size={12} />
                        ) : (
                          <ArrowDown size={12} />
                        )
                      ) : (
                        <ArrowUpDown size={11} className="sort-inactive" />
                      )}
                    </button>
                  ) : (
                    h.label
                  )}
                </th>
              ))}
            </>
          </tr>
        </thead>
        <tbody>
          {grouped
            ? [...groups.entries()].map(([key, entries]) => (
                <GroupedRows key={key} entries={entries} row={row} />
              ))
            : ports.map(row)}
        </tbody>
      </table>
      {!ports.length && (
        <div className="empty-state">
          {query ? <SearchX size={30} /> : <Network size={30} />}
          <h3>{query ? "No matching ports" : "Quiet seas."}</h3>
          <p>
            {query
              ? `Nothing in this view matches “${query}”. Try a different search or filter.`
              : "No listening ports were found in this view."}
          </p>
        </div>
      )}
    </div>
  );
}
function GroupedRows({
  entries,
  row,
}: {
  entries: PortEntry[];
  row: (p: PortEntry) => React.ReactNode;
}) {
  return (
    <>
      <tr className="group-header">
        <td colSpan={9}>
          <ProjectBadge entry={entries[0]} compact />
          <span className="muted mono">
            PID {entries[0].pid ?? "unavailable"}
          </span>
          <span className="group-count">
            {entries.length} {entries.length === 1 ? "port" : "ports"}
          </span>
        </td>
      </tr>
      {entries.map(row)}
    </>
  );
}
