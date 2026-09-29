import { CircleHelp, Search, ShieldCheck, X } from "lucide-react";
import { desktop } from "../lib/api";
import { Modal } from "./Modal";
export function HelpDialog({ closeHelp }: { closeHelp: () => void }) {
  return (
    <Modal
      close={closeHelp}
      label="Help and keyboard shortcuts"
      className="help-backdrop"
    >
      <div className="help-modal-title">
        <CircleHelp size={21} />
        <h2>A few useful shortcuts.</h2>
        <button
          className="icon-button"
          onClick={closeHelp}
          aria-label="Close help"
        >
          <X size={18} />
        </button>
      </div>
      <div className="shortcuts">
        {[
          ["Command palette", "⌘ / Ctrl K"],
          ["Search and filter", "⌘ / Ctrl F"],
          ["Refresh ports", "⌘ / Ctrl R"],
          ["Open settings", "⌘ / Ctrl ,"],
          ["Close dialog or drawer", "Esc"],
        ].map(([name, key]) => (
          <div key={name}>
            <span>{name}</span>
            <kbd>{key}</kbd>
          </div>
        ))}
      </div>
      <div className="help-tip">
        <Search size={17} />
        <p>
          Search <code>:5173</code> for an exact port match. Search for a free
          port to add it to your watchlist.
        </p>
      </div>
      <p className="help-description">
        {desktop
          ? "Port Authority inspects local sockets using native OS APIs. Processes belonging to other users and critical system services are protected."
          : "You’re exploring the browser preview with sample services. Run npm run desktop from the project to inspect your machine’s real ports using the Rust backend."}
      </p>
      <div className="help-tip">
        <ShieldCheck size={17} />
        <p>
          No accounts, analytics, or remote backend. Your workspace stays yours.
        </p>
      </div>
    </Modal>
  );
}
