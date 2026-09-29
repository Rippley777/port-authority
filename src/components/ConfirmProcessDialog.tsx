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
  return (
    <Modal
      close={closeConfirmation}
      label="Confirm process termination"
      className="confirm-backdrop"
    >
      <div className="confirmation-icon">
        <Square size={23} />
      </div>
      <h2>
        {confirmation.action === "force" ? "Force kill" : "Stop"}{" "}
        {confirmation.entry.process}?
      </h2>
      <p>
        {confirmation.action === "force"
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
          {confirmation.action === "force"
            ? "Force kill process"
            : "Stop process"}
        </button>
      </div>
    </Modal>
  );
}
