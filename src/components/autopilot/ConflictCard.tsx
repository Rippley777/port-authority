import { ProjectBadge } from "../../features/projects/ProjectBadge";
import { useProjectContext } from "../../features/projects/useProjects";
import {
  AlertTriangle,
  ArrowRight,
  Check,
  ChevronDown,
  CircleCheck,
  ExternalLink,
  LoaderCircle,
  RotateCw,
  Shield,
  Terminal,
  X,
} from "lucide-react";
import { useCallback, useState } from "react";
import type { Conflict, ConflictAction } from "../../lib/autopilot";
import { commandText, isPending, needsConfirmation } from "../../lib/autopilot";
import type { PortEntry } from "../../lib/types";
import { uptime } from "../../lib/ports";
import { Modal } from "../Modal";
import { ServiceIcon } from "../ServiceIcon";
interface Props {
  conflict: Conflict;
  busy: boolean;
  inspect: (p: PortEntry) => void;
  run: (
    id: string,
    action: ConflictAction,
    approved?: boolean,
    targetPort?: number | null,
  ) => Promise<void>;
}
export function ConflictCard({ conflict: c, busy, inspect, run }: Props) {
  const projects = useProjectContext();
  const ownerProject = c.owner?.project;
  const failedProject = projects.projects.find(
    (p) => p.identity.rootPath === c.cwd,
  )?.identity;
  const [confirmation, setConfirmation] = useState<ConflictAction | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const close = useCallback(() => {
    setConfirmation(null);
    setAcknowledged(false);
  }, []);
  const pending = isPending(c);
  const working = ["resolving", "starting"].includes(c.status);
  const resolved = c.status === "resolved";
  const blocked = c.safety.risk === "BLOCKED";
  function request(action: ConflictAction) {
    if (needsConfirmation(c, action)) {
      setConfirmation(action);
      setAcknowledged(false);
    } else void run(c.id, action, false, c.alternative);
  }
  const needsAcknowledgement =
    c.safety.risk === "HIGH" || confirmation === "force_retry";
  return (
    <article
      className={`conflict-card ${resolved ? "resolved" : ""}`}
      aria-label={`Conflict on port ${c.port}`}
    >
      <div className="conflict-title">
        <span className="conflict-symbol">
          {resolved ? (
            <CircleCheck size={21} />
          ) : working ? (
            <LoaderCircle size={21} className="spin" />
          ) : (
            <AlertTriangle size={21} />
          )}
        </span>
        <div>
          <span className="section-label">
            {resolved
              ? "RECOVERY VERIFIED"
              : working
                ? "RECOVERY IN PROGRESS"
                : "PORT CONFLICT"}
          </span>
          <h2>
            {resolved
              ? `${c.recoveryAction === "restart_owner" ? "Owner restarted" : "Back to building"} on port ${c.recoveryPort ?? c.port}`
              : c.status === "unverified"
                ? "Command running; port unverified"
                : c.status === "failed"
                  ? "Recovery needs attention"
                  : c.status === "expired"
                    ? "Captured command expired"
                    : c.status === "starting"
                      ? `Waiting for port ${c.recoveryPort ?? c.port}`
                      : c.status === "resolving"
                        ? `Checking recovery for port ${c.port}`
                        : `Port ${c.port} is already occupied`}
          </h2>
        </div>
        <button
          className="icon-button"
          aria-label={`Ignore conflict on port ${c.port}`}
          title="Ignore — leaves all processes running"
          disabled={busy || working}
          onClick={() => {
            void run(c.id, "ignore");
          }}
        >
          <X size={17} />
        </button>
      </div>
      <p className="conflict-message">{c.message}</p>
      <div className="conflict-command">
        <Terminal size={15} />
        <code>{commandText(c.command)}</code>
        <span title={c.cwd}>{failedProject?.name ?? c.project}</span>
      </div>
      {ownerProject && (
        <p className="conflict-project-context">
          {failedProject?.name ?? c.project} couldn’t start on :{c.port}.{" "}
          <strong>{ownerProject.name}</strong> is already using it.{" "}
          <button
            className="text-button"
            onClick={() => projects.open(ownerProject)}
          >
            Open {ownerProject.name}
          </button>
        </p>
      )}
      {c.owner ? (
        <div className="conflict-owner">
          <ServiceIcon entry={c.owner} large />
          <div>
            {c.owner.project ? (
              <ProjectBadge entry={c.owner} />
            ) : (
              <strong>{c.safety.project ?? "Project unavailable"}</strong>
            )}
            <p>
              {c.owner.process} <span>·</span> PID {c.owner.pid} <span>·</span>{" "}
              running {uptime(c.owner.startedAt)}
            </p>
          </div>
          {c.safety.possiblyStale && (
            <span
              className="stale-label"
              title="Heuristic: older than three hours and belongs to a different project. Age alone does not establish staleness."
            >
              Possibly left over
            </span>
          )}
        </div>
      ) : (
        <div className="conflict-owner unknown">
          <Shield size={22} />
          <p>No single identifiable owner. Nothing will be killed.</p>
        </div>
      )}
      {pending && (
        <div className={`conflict-safety risk-${c.safety.risk.toLowerCase()}`}>
          <Shield size={15} />
          <div>
            <strong>{c.safety.heading}</strong>
            <p>{c.safety.reasons[0]}</p>
            {c.safety.differentProject && (
              <p>
                Owner project: <code>{c.safety.projectPath}</code>
              </p>
            )}
          </div>
        </div>
      )}
      {c.steps.length > 0 && (
        <ol className="recovery-steps">
          {c.steps.map((step, i) => (
            <li key={`${i}-${step}`}>
              <Check size={14} />
              <span>{step}</span>
            </li>
          ))}
        </ol>
      )}
      {resolved && c.listener && (
        <div className="conflict-actions">
          <span className="mono">
            PID {c.listener.pid} · {c.listener.address}:{c.listener.port}
          </span>
          <button
            className="button"
            onClick={() => c.listener && inspect(c.listener)}
          >
            Inspect recovered process
            <ExternalLink size={13} />
          </button>
        </div>
      )}
      {working && (
        <div className="recovery-working" role="status">
          <LoaderCircle size={14} className="spin" />
          Checking actual process and socket state…
        </div>
      )}
      {c.status === "force_required" && (
        <p className="force-explanation">
          Graceful termination did not free the port. Force termination will
          never happen automatically.
        </p>
      )}
      {pending && (
        <div className="conflict-actions">
          {c.status === "force_required" ? (
            <button
              className="button danger"
              disabled={busy || blocked}
              onClick={() => request("force_retry")}
            >
              Force Kill & Retry…
            </button>
          ) : (
            <button
              className="button primary"
              disabled={busy || blocked}
              title={
                blocked
                  ? c.safety.reasons.join(" ")
                  : "Verify ownership, gracefully stop the owner, then retry your captured command"
              }
              onClick={() => request("kill_retry")}
            >
              Kill & Retry
              <ArrowRight size={14} />
            </button>
          )}
          <button
            className="button"
            disabled={busy || !c.alternative}
            title={c.alternateReason}
            onClick={() => request("alternate")}
          >
            {c.alternative
              ? `Use ${c.alternative} Instead`
              : "Alternate port unavailable"}
          </button>
          <button
            className="button"
            disabled={!c.owner || busy}
            onClick={() => c.owner && inspect(c.owner)}
          >
            <ExternalLink size={13} />
            Inspect
          </button>
          <button
            className="text-button"
            disabled={busy || !c.restartOwnerAvailable}
            title={c.restartOwnerReason}
            onClick={() => request("restart_owner")}
          >
            <RotateCw size={13} />
            Restart Owner
          </button>
        </div>
      )}
      {pending && (
        <div className="conflict-secondary">
          <button
            className="text-button"
            disabled={busy}
            onClick={() => request("retry")}
          >
            Owner already stopped? Retry without killing
          </button>
          <span>
            {c.safety.previouslyObserved
              ? "Owner previously observed"
              : "Owner first observed at conflict"}
          </span>
        </div>
      )}
      <details className="conflict-evidence">
        <summary>
          <ChevronDown size={13} />
          Command context & diagnostic output
        </summary>
        <dl>
          <dt>Working directory</dt>
          <dd>{c.cwd}</dd>
          <dt>Observed owner command</dt>
          <dd>
            {c.owner?.command.length
              ? commandText(c.owner.command)
              : "Unavailable"}
          </dd>
          {c.retryCommand && (
            <>
              <dt>Executed retry command</dt>
              <dd>{commandText(c.retryCommand)}</dd>
            </>
          )}
          <dt>Captured context</dt>
          <dd>
            Arguments, executable, directory, and exported environment.
            Environment values stay in native memory.
          </dd>
          <dt>Pending capture expires</dt>
          <dd>{new Date(c.expiresAt * 1000).toLocaleTimeString()}</dd>
        </dl>
        <pre>{c.evidence}</pre>
        {c.output && (
          <>
            <h4>Retry output</h4>
            <pre>{c.output}</pre>
          </>
        )}
        {c.safety.reasons.slice(1).map((reason) => (
          <p key={reason}>{reason}</p>
        ))}
      </details>
      {confirmation && (
        <Modal
          label="Confirm conflict recovery"
          close={close}
          className="confirm-backdrop"
        >
          <div className="confirmation-icon">
            <AlertTriangle size={24} />
          </div>
          <h2>
            {confirmation === "force_retry"
              ? "Force stop this owner?"
              : confirmation === "restart_owner"
                ? "Restart the owning project?"
                : "Stop this project’s process?"}
          </h2>
          <p>
            {confirmation === "restart_owner"
              ? c.restartOwnerReason
              : c.safety.reasons.join(" ")}
          </p>
          <div className="confirm-process">
            <Terminal size={16} />
            <span>{c.owner?.process}</span>
            <span className="mono">PID {c.owner?.pid}</span>
          </div>
          <p className="confirm-note">
            Owner: {c.safety.projectPath ?? "Project unavailable"}
            <br />
            Failed command: {commandText(c.command)}
            <br />
            Retry directory: {c.cwd}
          </p>
          <p className="confirm-note">
            {confirmation === "force_retry"
              ? "A forced stop can discard unsaved work. The PID, start time, and port ownership will be checked again."
              : "Other ports and services owned by this process will also be interrupted."}
          </p>
          {needsAcknowledgement && (
            <label className="risk-acknowledgement">
              <input
                type="checkbox"
                checked={acknowledged}
                onChange={(e) => setAcknowledged(e.target.checked)}
              />
              I understand this may interrupt other applications or lose unsaved
              work.
            </label>
          )}
          <div className="modal-actions">
            <button className="button" onClick={close}>
              Cancel
            </button>
            <button
              className="button danger-solid"
              disabled={needsAcknowledgement && !acknowledged}
              onClick={() => {
                const action = confirmation;
                close();
                void run(c.id, action, true, c.alternative);
              }}
            >
              {confirmation === "force_retry"
                ? "Confirm Force Kill & Retry"
                : confirmation === "restart_owner"
                  ? "Confirm Restart Owner"
                  : "Confirm Kill & Retry"}
            </button>
          </div>
        </Modal>
      )}
    </article>
  );
}
