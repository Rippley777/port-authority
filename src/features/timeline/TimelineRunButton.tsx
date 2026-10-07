import type { PortEntry, ProcessAction } from "../../lib/types";
import { activeLaunch } from "../../lib/runHistory";
import { Play, RotateCw } from "lucide-react";
import { useRef, useState } from "react";
import type { DisplayLaunchContext } from "../recovery/types";
import {
  getHistoricalLaunchContext,
  historicalCommandNeedsConfirmation,
  runHistoricalCommand,
} from "../recovery/api";
import { RunAgainDialog } from "../recovery/RunAgainDialog";
import { RecoveryProgress } from "../recovery/LaunchSection";

export function TimelineRunButton({
  launch,
  ports,
  requestAction,
  finished,
}: {
  launch?: DisplayLaunchContext | null;
  ports: PortEntry[];
  requestAction: (entry: PortEntry, action: ProcessAction) => void;
  finished: (message: string, error?: boolean) => void;
}) {
  const active = activeLaunch(launch, ports);
  const running = useRef(false);
  const [busy, setBusy] = useState(false);
  const [confirmation, setConfirmation] = useState<DisplayLaunchContext>();
  const [feedback, setFeedback] = useState("");
  const [failed, setFailed] = useState(false);
  const [started, setStarted] = useState(false);
  const unavailable =
    "The original command was not saved for this timeline item.";

  async function run(confirmed = false) {
    if (!launch?.id || running.current) return;
    running.current = true;
    setBusy(true);
    setFailed(false);
    setFeedback("Checking saved command…");
    try {
      // Snapshot availability can be stale. Resolve the current saved context by ID.
      const context = await getHistoricalLaunchContext(launch.id);
      if (!context?.recoverable)
        throw new Error(context?.reason || unavailable);
      if (historicalCommandNeedsConfirmation(context) && !confirmed) {
        setConfirmation(context);
        setFeedback("");
        return;
      }
      setFeedback("Starting…");
      const result = await runHistoricalCommand(context.id, confirmed);
      setStarted(true);
      setFeedback(result.message);
      finished(result.message);
    } catch (reason) {
      const message = String(reason);
      setFailed(true);
      setFeedback(message);
      finished(message, true);
    } finally {
      running.current = false;
      setBusy(false);
    }
  }

  if (active)
    return (
      <div className="timeline-run">
        <button
          className="button timeline-run-button"
          aria-label="Restart running command"
          title={
            active.restartable
              ? `Restart on :${active.port}`
              : active.restartReason
          }
          disabled={!active.restartable || active.protected || !active.pid}
          onClick={() => requestAction(active, "restart")}
        >
          <RotateCw size={13} /> Restart
        </button>
      </div>
    );

  return (
    <div className="timeline-run">
      <button
        className="button timeline-run-button"
        aria-label="Run command again"
        title={launch?.id ? `Run again: ${launch.command}` : unavailable}
        disabled={!launch?.id || busy || !!confirmation}
        onClick={() => void run()}
      >
        {busy ? (
          <RotateCw size={13} className="spin" />
        ) : (
          <Play size={13} fill="currentColor" />
        )}
        {busy ? "Starting…" : "Run Again"}
      </button>
      {feedback && <p role={failed ? "alert" : "status"}>{feedback}</p>}
      {started && <RecoveryProgress id={launch?.id} />}
      {confirmation && (
        <RunAgainDialog
          command={{ launchContext: confirmation }}
          close={() => setConfirmation(undefined)}
          confirm={() => {
            setConfirmation(undefined);
            void run(true);
          }}
        />
      )}
    </div>
  );
}
