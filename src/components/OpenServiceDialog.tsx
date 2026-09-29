import { ExternalLink } from "lucide-react";
import type { PortEntry, Settings } from "../lib/types";
import { Modal } from "./Modal";
export function OpenServiceDialog({
  manualOpen,
  closeManual,
  settings,
  doOpen,
}: {
  manualOpen: PortEntry;
  closeManual: () => void;
  settings: Settings;
  doOpen: (p: PortEntry) => Promise<void>;
}) {
  return (
    <Modal
      close={closeManual}
      label="Open service in browser"
      className="confirm-backdrop"
    >
      <div className="confirmation-icon neutral">
        <ExternalLink size={23} />
      </div>
      <h2>Open port {manualOpen.port}?</h2>
      <p>
        This service isn’t recognized as an HTTP development server. It may not
        respond to browser requests.
      </p>
      <div className="code-block">
        {settings.protocol}://localhost:{manualOpen.port}
      </div>
      <div className="modal-actions">
        <button className="button" onClick={closeManual}>
          Cancel
        </button>
        <button
          className="button primary"
          onClick={() => {
            void doOpen(manualOpen);
            closeManual();
          }}
        >
          Open anyway
          <ExternalLink size={14} />
        </button>
      </div>
    </Modal>
  );
}
