import {
  AlertCircle,
  ArrowRight,
  Check,
  Copy,
  Eye,
  Radar,
  ShieldCheck,
  Terminal,
  X,
} from "lucide-react";
import { useState } from "react";
import type { AutopilotController } from "../../hooks/useAutopilot";
import type { PortEntry } from "../../lib/types";
import type { PreviewScenario } from "../../lib/autopilot";
import { desktop } from "../../lib/api";
import { Toggle } from "../Screens";
import { ConflictCard } from "./ConflictCard";
export function AutopilotPage({
  controller: a,
  inspect,
  copy,
}: {
  controller: AutopilotController;
  inspect: (p: PortEntry) => void;
  copy: (value: string, label: string) => Promise<void>;
}) {
  const [scenario, setScenario] = useState<PreviewScenario>("dev");
  const snapshot = a.snapshot;
  return (
    <div className="autopilot-page">
      <div className="autopilot-intro">
        <div className="autopilot-emblem">
          <Radar size={27} />
        </div>
        <div>
          <h2>A clear path back to your dev server.</h2>
          <p>
            Know what’s blocking your command. Recover with context and control.
          </p>
        </div>
        <span
          className={`autopilot-state ${snapshot?.enabled ? "enabled" : ""}`}
        >
          <span className="status-dot" />
          {snapshot?.enabled
            ? "Watching captured commands"
            : "Shell capture is off"}
        </span>
      </div>
      {a.error && (
        <div className="error-banner" role="alert">
          <AlertCircle size={17} />
          <p>{a.error}</p>
          <button
            className="icon-button"
            aria-label="Dismiss Autopilot error"
            onClick={a.clearError}
          >
            <X size={15} />
          </button>
        </div>
      )}
      {snapshot?.connectionError && (
        <div className="error-banner" role="alert">
          <AlertCircle size={17} />
          <p>{snapshot.connectionError}</p>
        </div>
      )}
      {!snapshot && (
        <div className="panel empty-state">
          <Radar size={27} />
          <p>Connecting to Conflict Autopilot…</p>
        </div>
      )}
      {snapshot && (
        <>
          <section className="panel autopilot-setup">
            <div className="panel-heading">
              <div>
                <Terminal size={17} />
                <h3>
                  {desktop
                    ? "Optional shell integration"
                    : "Explore Conflict Autopilot"}
                </h3>
                <span className="count-badge">
                  {desktop ? "BASH + ZSH" : "PREVIEW"}
                </span>
              </div>
              {desktop && (
                <Toggle
                  label="Enable Conflict Autopilot"
                  checked={snapshot.enabled}
                  onChange={() => {
                    void a.toggle(!snapshot.enabled);
                  }}
                  disabled={
                    a.busy || !snapshot.supported || !!snapshot.connectionError
                  }
                />
              )}
            </div>
            {desktop ? (
              <div className="setup-body">
                <p>
                  {snapshot.supported
                    ? "Enable capture, then run this in a bash or zsh session. Only conventional dev/start commands are intercepted; other commands run normally."
                    : "Shell integration currently supports macOS and Linux with bash or zsh. Windows shell capture is not available yet."}
                </p>
                {snapshot.supported && (
                  <>
                    <div className="setup-command">
                      <pre>{snapshot.setupCommand}</pre>
                      <button
                        className="button"
                        disabled={
                          !snapshot.enabled || !!snapshot.connectionError
                        }
                        onClick={() => {
                          void copy(snapshot.setupCommand, "Shell setup");
                        }}
                      >
                        <Copy size={14} />
                        Copy setup
                      </button>
                    </div>
                    <div className="setup-options">
                      <span>
                        <Check size={13} />
                        No startup files modified
                      </span>
                      <span>
                        <ShieldCheck size={13} />
                        Local, same-user connection
                      </span>
                      <span>
                        <Eye size={13} />
                        No background terminal scraping
                      </span>
                    </div>
                    <p className="setup-footnote">
                      For explicit capture, source the script and use{" "}
                      <code>pa npm run dev</code> without calling{" "}
                      <code>pa_autopilot_on</code>. Run{" "}
                      <code>pa_autopilot_off</code> to restore normal commands
                      in this session. Turning capture off clears pending
                      contexts; running services continue.
                    </p>
                  </>
                )}
              </div>
            ) : (
              <div className="setup-body preview-setup">
                <div>
                  <p>
                    Try a sample conflict. No real command will run and no
                    process will be stopped.
                  </p>
                  <span className="setup-footnote">
                    Open the desktop app to connect your actual shell.
                  </span>
                </div>
                <select
                  aria-label="Conflict preview scenario"
                  value={scenario}
                  onChange={(e) =>
                    setScenario(e.target.value as PreviewScenario)
                  }
                >
                  <option value="dev">Vite · another project</option>
                  <option value="database">PostgreSQL · infrastructure</option>
                  <option value="unresponsive">
                    Dev server · ignores SIGTERM
                  </option>
                </select>
                <button
                  className="button primary"
                  disabled={a.busy}
                  onClick={() => {
                    void a.simulate(scenario);
                  }}
                >
                  Simulate a conflict
                  <ArrowRight size={14} />
                </button>
              </div>
            )}
          </section>
          {snapshot.conflicts.length > 0 ? (
            <div className="conflict-list">
              {snapshot.conflicts.map((c) => (
                <ConflictCard
                  key={c.id}
                  conflict={c}
                  busy={a.busy}
                  inspect={inspect}
                  run={a.run}
                />
              ))}
            </div>
          ) : (
            <section className="panel autopilot-empty">
              <div className="radar-illustration">
                <Radar size={39} />
              </div>
              <h3>
                {snapshot.enabled
                  ? "Nothing in your way."
                  : "Your next port conflict, handled."}
              </h3>
              <p>
                {snapshot.enabled
                  ? "When a captured command fails with an occupied-port error, the owner and safe recovery actions will appear here."
                  : "Opt in to shell capture to turn an EADDRINUSE error into a clear next step."}
              </p>
              <div className="autopilot-flow">
                <span>Command fails</span>
                <ArrowRight size={13} />
                <span>Owner identified</span>
                <ArrowRight size={13} />
                <span>You choose recovery</span>
              </div>
            </section>
          )}
          <div className="autopilot-principles">
            <ShieldCheck size={16} />
            <p>
              Nothing is killed automatically. Every retry rechecks live
              ownership. Pending captures expire after 15 minutes; environments
              never enter browser storage or logs. Retried commands run under
              Port Authority with captured output. Launch metadata is saved
              locally; private environment values stay in memory. Relaunched
              commands continue running when the app closes.
            </p>
          </div>
        </>
      )}
    </div>
  );
}
export function ConflictBanner({
  controller: a,
  open,
}: {
  controller: AutopilotController;
  open: () => void;
}) {
  const conflicts =
    a.snapshot?.conflicts.filter((c) =>
      [
        "detected",
        "force_required",
        "resolving",
        "starting",
        "failed",
        "unverified",
      ].includes(c.status),
    ) ?? [];
  if (!conflicts.length) return null;
  const c = conflicts[0];
  return (
    <div className="autopilot-banner" role="status">
      <AlertCircle size={20} />
      <div>
        <strong>
          {["failed", "unverified"].includes(c.status)
            ? `Recovery for ${c.project} needs attention`
            : `Port ${c.port} is blocking ${c.project}`}
        </strong>
        <p>
          {c.safety.project ?? c.owner?.process ?? "Owner unverified"}
          {c.owner?.pid ? ` · PID ${c.owner.pid}` : ""}
          {conflicts.length > 1
            ? ` · ${conflicts.length} conflicts to review`
            : ""}
        </p>
      </div>
      <button className="button" onClick={open}>
        Review conflict
        <ArrowRight size={14} />
      </button>
    </div>
  );
}
