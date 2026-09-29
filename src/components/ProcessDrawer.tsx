import {
  Copy,
  ExternalLink,
  FolderOpen,
  RotateCw,
  Shield,
  Square,
  Star,
  X,
} from "lucide-react";
import type { PortEntry, ProcessAction } from "../lib/types";
import { connectionAddress, service, uptime } from "../lib/ports";
import { ServiceIcon } from "./ServiceIcon";
import { Modal } from "./Modal";
interface Props {
  entry: PortEntry;
  close: () => void;
  favorite: boolean;
  toggleFavorite: () => void;
  copy: (text: string, label: string) => void;
  open: () => void;
  control: (action: ProcessAction) => void;
  reveal: () => void;
}
export function ProcessDrawer({
  entry: p,
  close,
  favorite,
  toggleFavorite,
  copy,
  open,
  control,
  reveal,
}: Props) {
  const details = [
    ["Process ID", p.pid],
    ["Parent process", p.parentPid],
    ["User", p.user],
    [
      "Started",
      p.startedAt ? new Date(p.startedAt * 1000).toLocaleString() : null,
    ],
    ["Uptime", p.startedAt ? uptime(p.startedAt) : null],
    [
      "Memory",
      p.memory !== null ? `${(p.memory / 1024 / 1024).toFixed(1)} MB` : null,
    ],
    ["CPU", p.cpu !== null ? `${p.cpu.toFixed(1)}%` : null],
  ];
  return (
    <Modal
      label={`Process details for ${p.process}`}
      close={close}
      className="drawer-backdrop"
    >
      <div className="drawer-top">
        <span>PROCESS DETAILS</span>
        <button
          className="icon-button"
          aria-label="Close details"
          onClick={close}
        >
          <X size={19} />
        </button>
      </div>
      <div className="drawer-body">
        <div className="drawer-identity">
          <ServiceIcon entry={p} large />
          <div>
            <h2>{p.process}</h2>
            <p>{service(p).label}</p>
          </div>
          <button
            className={`star-button ${favorite ? "is-favorite" : ""}`}
            onClick={toggleFavorite}
            title="Watch this port"
          >
            <Star size={19} />
          </button>
        </div>
        <div className="drawer-status">
          <span className="listening">
            <span className="status-dot" />
            {p.protocol === "TCP" ? "Listening" : "Bound"}
          </span>
          <span className="mono">:{p.port}</span>
          <span className="protocol">{p.protocol}</span>
        </div>
        <div className="drawer-action-bar">
          <button className="button primary" onClick={open}>
            <ExternalLink size={14} />
            Open in browser
          </button>
          <button
            className="button"
            disabled={!p.restartable}
            title={p.restartReason}
            onClick={() => control("restart")}
          >
            <RotateCw size={14} />
            Restart
          </button>
        </div>
        {p.permissionLimited && (
          <div className="permission-note">
            <Shield size={16} />
            <p>
              Some process information is unavailable because Port Authority
              does not have permission to inspect this process. Run the app as
              the process owner to see more details.
            </p>
          </div>
        )}
        <h4 className="section-label">CONNECTION</h4>
        <dl className="details">
          <dt>Local address</dt>
          <dd className="mono">
            {connectionAddress(p)}
            <button
              className="icon-button"
              title="Copy address"
              onClick={() => copy(connectionAddress(p), "Address")}
            >
              <Copy size={13} />
            </button>
          </dd>
          <dt>Protocol</dt>
          <dd>
            {p.protocol === "TCP"
              ? "TCP · listening socket"
              : "UDP · bound socket"}
          </dd>
          {p.protocol === "TCP" && (
            <>
              <dt>Remote address</dt>
              <dd className="muted">Not connected</dd>
            </>
          )}
        </dl>
        <h4 className="section-label">PROCESS</h4>
        <dl className="details">
          {details
            .filter(([, value]) => value !== null)
            .map(([key, value]) => (
              <div className="detail-pair" key={key}>
                <dt>{key}</dt>
                <dd
                  className={
                    key === "Process ID" || key === "Parent process"
                      ? "mono"
                      : ""
                  }
                >
                  {value}
                  {key === "Process ID" && (
                    <button
                      className="icon-button"
                      title="Copy PID"
                      onClick={() => copy(String(value), "PID")}
                    >
                      <Copy size={13} />
                    </button>
                  )}
                </dd>
              </div>
            ))}
        </dl>
        {p.command.length > 0 && (
          <>
            <h4 className="section-label">
              COMMAND
              <button
                className="icon-button"
                title="Copy command"
                onClick={() => copy(p.command.join(" "), "Command")}
              >
                <Copy size={13} />
              </button>
            </h4>
            <div className="code-block">{p.command.join(" ")}</div>
            {p.command.length > 1 && (
              <>
                <h4 className="section-label">ARGUMENTS</h4>
                <div className="code-block">
                  {p.command.slice(1).join("\n")}
                </div>
              </>
            )}
          </>
        )}
        {p.cwd && (
          <>
            <h4 className="section-label">WORKING DIRECTORY</h4>
            <div className="code-block">{p.cwd}</div>
          </>
        )}
        {p.executable && (
          <>
            <h4 className="section-label">
              EXECUTABLE
              <button
                className="icon-button"
                title="Reveal executable"
                onClick={reveal}
              >
                <FolderOpen size={14} />
              </button>
            </h4>
            <div className="code-block">{p.executable}</div>
          </>
        )}
        {!p.restartable && (
          <p className="drawer-footnote">
            Restart unavailable: {p.restartReason}
          </p>
        )}
      </div>
      <div className="drawer-footer">
        {p.protected ? (
          <span className="protected-label">
            <Shield size={15} />
            Protected system process
          </span>
        ) : (
          <>
            <button
              className="button danger"
              disabled={!p.pid}
              onClick={() => control("kill")}
            >
              <Square size={13} />
              Kill process
            </button>
            <button
              className="text-button danger-text"
              disabled={!p.pid}
              onClick={() => control("force")}
            >
              Force kill…
            </button>
          </>
        )}
      </div>
    </Modal>
  );
}
