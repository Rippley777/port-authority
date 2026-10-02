import { AlertTriangle, Play } from "lucide-react";
import { Modal } from "../../components/Modal";
import type { HistoricalCommand } from "./types";

export function RunAgainDialog({
  command,
  close,
  confirm,
}: {
  command: HistoricalCommand;
  close: () => void;
  confirm: () => void;
}) {
  return (
    <Modal close={close} label="Confirm historical command">
      <div className="confirm-icon warning">
        <AlertTriangle size={22} />
      </div>
      <h2>Run this recovered command?</h2>
      <p className="confirm-description">
        This command is outside Port Authority’s standard development-command
        adapters. Review the exact launch context before continuing.
      </p>
      <dl className="details run-confirm-details">
        <dt>Command</dt>
        <dd className="mono">{command.launchContext.command}</dd>
        <dt>Working directory</dt>
        <dd className="mono">{command.launchContext.workingDirectory}</dd>
      </dl>
      <div className="dialog-actions">
        <button className="button" onClick={close}>
          Cancel
        </button>
        <button className="button primary" onClick={confirm}>
          <Play size={13} fill="currentColor" /> Run Command
        </button>
      </div>
    </Modal>
  );
}
