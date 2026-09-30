import { RecoveryProgress } from "../features/recovery/LaunchSection";
import { Info, LoaderCircle, Square, Terminal } from "lucide-react";
import type { PortEntry, ProcessAction } from "../lib/types";
import { desktop } from "../lib/api";
import { Modal } from "./Modal";
export function ConfirmProcessDialog({
  confirmation,
  closeConfirmation,
  acting,
  ports,
  perform,
}: {
  confirmation: { entry: PortEntry; action: ProcessAction };
  closeConfirmation: () => void;
  acting: boolean;
  ports: PortEntry[];
  perform: (p: PortEntry, a: ProcessAction) => Promise<void>;
}) {
  const recovering =
    confirmation.action === "restart" || confirmation.action === "relaunch";
  const label =
    confirmation.action === "restart"
      ? "Restart"
      : confirmation.action === "relaunch"
        ? "Kill & Relaunch"
        : confirmation.action === "force"
          ? "Force kill"
          : "Stop";
  return (
    <Modal
      close={closeConfirmation}
      label={
        recovering ? "Confirm command recovery" : "Confirm process termination"
      }
      className="confirm-backdrop"
    >
      <div className="confirmation-icon">
        <Square size={23} />
      </div>
      <h2>
        {label} {confirmation.entry.process}?
      </h2>
      <p>
        {recovering
          ? confirmation.action === "relaunch"
            ? "Force stops the launch root and its children, then runs the recovery command. Unsaved work may be lost."
            : "Gracefully stops the launch root and its children, then runs the original command and verifies its ports."
          : confirmation.action === "force"
            ? "This immediately stops the process without cleanup. Unsaved work may be lost."
            : "This sends a graceful termination signal. Any services owned by this process will stop."}
      </p>
      <div className="confirm-process">
        <Terminal size={17} />
        <span>{confirmation.entry.process}</span>
        <span className="mono">PID {confirmation.entry.pid}</span>
      </div>
      <p className="confirm-note">
        Affected ports:{" "}
        {ports
          .filter((p) => p.pid === confirmation.entry.pid)
          .map((p) => `:${p.port}`)
          .join(", ")}
      </p>
      {recovering && (
        <>
          <div className="code-block">
            {confirmation.entry.launch?.command}
            <br />
            {confirmation.entry.launch?.workingDirectory}
          </div>
          <p className="confirm-note">
            Launch root: PID {confirmation.entry.launch?.launchRoot.pid}. All
            ports owned by this launch are affected.
          </p>
          <RecoveryProgress id={confirmation.entry.launch?.id} />
        </>
      )}
      {!desktop && (
        <div className="preview-warning">
          <Info size={15} />
          Preview only. No real process will be affected.
        </div>
      )}
      <div className="modal-actions">
        <button
          className="button"
          onClick={closeConfirmation}
          disabled={acting}
        >
          Cancel
        </button>
        <button
          className="button danger-solid"
          disabled={acting}
          onClick={() => {
            void perform(confirmation.entry, confirmation.action);
          }}
        >
          {acting ? (
            <LoaderCircle size={15} className="spin" />
          ) : (
            <Square size={12} />
          )}
          {recovering ? label : `${label} process`}
        </button>
      </div>
    </Modal>
  );
}
