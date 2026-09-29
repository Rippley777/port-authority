import { AlertTriangle, GitBranch, X } from "lucide-react";
import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop } from "../../lib/api";
import { Modal } from "../../components/Modal";
import { ProjectActions } from "../projects/ProjectActions";
import type { Recurrence, TreeMember } from "./types";
function treeDepth(member: TreeMember, tree: TreeMember[]) {
  const seen = new Set<number>();
  let parent = member.parentPid;
  while (parent && !seen.has(parent)) {
    const ancestor = tree.find((p) => p.identity.pid === parent);
    if (!ancestor) break;
    seen.add(parent);
    parent = ancestor.parentPid;
  }
  return Math.min(seen.size, 8);
}
export function RecurrencePanel({ recurrence: r }: { recurrence: Recurrence }) {
  const [tree, setTree] = useState<TreeMember[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [approved, setApproved] = useState(false);
  const [stopped, setStopped] = useState(false);
  const close = useCallback(() => {
    setTree(null);
    setApproved(false);
  }, []);
  const parent = r.persistentAncestor;
  const p = r.process.process;
  async function inspect() {
    if (!parent) return;
    setBusy(true);
    setError(null);
    setStopped(false);
    try {
      const result = desktop
        ? await invoke<TreeMember[]>("timeline_inspect_tree", {
            root: parent.identity,
          })
        : [
            {
              identity: parent.identity,
              name: parent.name,
              parentPid: parent.parentPid,
              command: parent.command,
            },
            {
              identity: { pid: p.pid!, startedAt: p.startedAt! },
              name: p.process,
              parentPid: parent.identity.pid,
              command: p.command,
            },
          ];
      setTree(result);
      setApproved(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function stop() {
    if (!parent || !tree || !approved) return;
    setBusy(true);
    setError(null);
    try {
      if (!desktop)
        throw new Error("Preview only. No real process was affected.");
      await invoke("timeline_stop_tree", {
        root: parent.identity,
        approved: tree.map((p) => p.identity),
      });
      setStopped(true);
      close();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="recurrence-panel">
      <div className="recurrence-title">
        <AlertTriangle size={19} />
        <h3>Port {r.port} keeps returning</h3>
        <span className="protocol">
          {r.protocol} · {r.address}
        </span>
      </div>
      <p>
        <strong>{r.count} restarts / reclaims</strong> observed between{" "}
        {new Date(r.firstAt * 1000).toLocaleTimeString()} and{" "}
        {new Date(r.latestAt * 1000).toLocaleTimeString()}.
      </p>
      <div className="recurrence-source">
        <div>
          <span className="section-label">
            {parent ? "LIKELY SOURCE" : "CURRENT SERVICE"}
          </span>
          <strong>
            {parent?.command.join(" ") || p.project?.name || p.process}
          </strong>
          <span>
            {p.project?.displayPath ?? p.cwd ?? "Working directory unavailable"}
          </span>
        </div>
        {parent && (
          <div>
            <span className="section-label">PERSISTENT ANCESTOR</span>
            <strong>
              {parent.name}{" "}
              <span className="mono">PID {parent.identity.pid}</span>
            </strong>
            <span>
              Started{" "}
              {new Date(parent.identity.startedAt * 1000).toLocaleString()}
            </span>
          </div>
        )}
      </div>
      <p>
        {parent
          ? `The same ${parent.name} process was observed in each owner’s ancestry. It may be supervising this service; stopping only the child may let it return.`
          : "Repeated activity is visible, but there is not enough ancestry evidence to identify a supervisor."}
      </p>
      <p className="muted">Observed PIDs: {r.previousPids.join(" → ")}</p>
      <div className="timeline-actions">
        {parent && (
          <>
            <button
              className="button"
              disabled={busy}
              onClick={() => void inspect()}
            >
              <GitBranch size={14} />
              Inspect Parent / Process Tree
            </button>
            <button
              className="button danger-text"
              disabled={busy}
              onClick={() => void inspect()}
            >
              Stop Process Tree…
            </button>
          </>
        )}
        {p.project && <ProjectActions project={p.project} />}
      </div>
      {error && !tree && (
        <p role="alert" className="danger-text">
          {error}
        </p>
      )}
      {stopped && (
        <p role="status">
          Termination requested for the reviewed process tree.
        </p>
      )}
      {tree && (
        <Modal label="Inspect process tree" close={close}>
          <div className="modal-heading">
            <h2>Inspect process tree</h2>
            <button
              className="icon-button"
              aria-label="Close process tree"
              onClick={close}
            >
              <X size={18} />
            </button>
          </div>
          <p>
            Live processes beneath {parent?.name}. Stopping this tree can
            interrupt every service listed here.
          </p>
          <div className="timeline-tree">
            {tree.map((member) => (
              <div
                key={`${member.identity.pid}:${member.identity.startedAt}`}
                style={{ marginLeft: treeDepth(member, tree) * 14 }}
              >
                <strong>
                  {member.identity.pid !== tree[0].identity.pid ? "└ " : ""}
                  {member.name} · PID {member.identity.pid}
                </strong>
                <small>Parent PID {member.parentPid ?? "unknown"}</small>
                <code>{member.command.join(" ")}</code>
              </div>
            ))}
          </div>
          <p>Previously observed listener PIDs: {r.previousPids.join(", ")}</p>
          <label className="tree-ack">
            <input
              type="checkbox"
              checked={approved}
              onChange={(e) => setApproved(e.target.checked)}
            />
            I want to stop all {tree.length} listed processes.
          </label>
          {error && <p role="alert">{error}</p>}
          <div className="modal-actions">
            <button className="button" onClick={close}>
              Cancel
            </button>
            <button
              className="button danger"
              disabled={!approved || busy}
              onClick={() => void stop()}
            >
              Confirm Stop Process Tree
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}
