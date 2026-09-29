import { useCallback, useState } from "react";
import { Trash2 } from "lucide-react";
import { Modal } from "../../components/Modal";
import { clearTimeline, configureTimeline } from "./api";
import { useTimeline } from "./useTimeline";
import type { TimelineConfig, TimelinePage } from "./types";
export function TimelineSettings() {
  const timeline = useTimeline({ limit: 1 });
  return (
    <section className="panel settings-panel">
      <h3>Port history</h3>
      <p className="muted">
        Process commands, projects, and paths stay in a local SQLite database.
      </p>
      {timeline.error && <p role="alert">{timeline.error}</p>}
      {timeline.data ? (
        <TimelineStorageControls
          data={timeline.data}
          reload={timeline.reset}
          settings
        />
      ) : (
        <p>Reading history settings…</p>
      )}
    </section>
  );
}
export function TimelineStorageControls({
  data,
  reload,
  settings = false,
}: {
  data: TimelinePage;
  reload: () => void;
  settings?: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const close = useCallback(() => setConfirm(false), []);
  async function update(change: Partial<TimelineConfig>) {
    setBusy(true);
    setError(null);
    try {
      await configureTimeline({ ...data.config, ...change });
      reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function clear() {
    setBusy(true);
    setError(null);
    try {
      await clearTimeline();
      close();
      reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="timeline-storage">
      {settings && (
        <>
          <div className="setting-row">
            <div>
              <strong>Record port history</strong>
              <p>Keep existing records when recording is disabled.</p>
            </div>
            <button
              role="switch"
              aria-label="Record port history"
              aria-checked={data.config.enabled}
              className={`toggle ${data.config.enabled ? "checked" : ""}`}
              disabled={busy}
              onClick={() => void update({ enabled: !data.config.enabled })}
            >
              <span />
            </button>
          </div>
          <div className="setting-row">
            <div>
              <strong>Keep history</strong>
              <p>Expired events are removed automatically.</p>
            </div>
            <select
              aria-label="History retention"
              disabled={busy}
              value={data.config.retentionDays}
              onChange={(e) =>
                void update({ retentionDays: Number(e.target.value) })
              }
            >
              {[
                [1, "24 hours"],
                [7, "7 days"],
                [30, "30 days"],
                [90, "90 days"],
                [0, "Forever"],
              ].map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </div>
          <div className="setting-row">
            <div>
              <strong>Recurring activity window</strong>
              <p>
                Flag at least {data.config.reclaimThreshold} restarts or
                reclaims in this interval.
              </p>
            </div>
            <select
              aria-label="Reclaim detection window"
              value={data.config.reclaimWindowSeconds}
              disabled={busy}
              onChange={(e) =>
                void update({ reclaimWindowSeconds: Number(e.target.value) })
              }
            >
              {[60, 300, 600, 900, 3600].map((n) => (
                <option key={n} value={n}>
                  {n / 60} minutes
                </option>
              ))}
            </select>
          </div>
        </>
      )}
      <div className="timeline-storage-footer">
        <span className="muted">
          {data.databaseBytes
            ? `${(data.databaseBytes / 1024 / 1024).toFixed(2)} MB on disk · `
            : ""}
          {data.config.retentionDays
            ? `${data.config.retentionDays === 1 ? "24 hours" : `${data.config.retentionDays} days`} retained`
            : "Retained forever"}{" "}
          · Local only
        </span>
        <button
          className="button"
          disabled={busy}
          onClick={() => setConfirm(true)}
        >
          <Trash2 size={13} />
          Clear History
        </button>
      </div>
      {error && <p role="alert">{error}</p>}
      {confirm && (
        <Modal label="Clear port history" close={close}>
          <h2>Clear port history?</h2>
          <p>
            This removes all saved events and monitoring sessions. Recording
            will continue from the next scan if enabled.
          </p>
          <div className="modal-actions">
            <button className="button" onClick={close}>
              Cancel
            </button>
            <button
              className="button danger"
              disabled={busy}
              onClick={() => void clear()}
            >
              Delete all history
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
