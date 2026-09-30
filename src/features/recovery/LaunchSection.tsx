import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Terminal } from "lucide-react";
import { desktop } from "../../lib/api";
import type { PortEntry } from "../../lib/types";
import {
  launchSources,
  type LaunchProfile,
  type RecoveryStatus,
} from "./types";

export function RecoveryProgress({ id }: { id?: string }) {
  const [status, setStatus] = useState<RecoveryStatus | null>(null);
  const [showOutput, setShowOutput] = useState(false);
  const [inspection, setInspection] = useState("");
  useEffect(() => {
    if (!id || !desktop) return;
    let disposed = false;
    const poll = () =>
      void invoke<RecoveryStatus | null>("recovery_status", { id })
        .then((s) => {
          if (!disposed) setStatus(s);
        })
        .catch(() => {});
    poll();
    const timer = setInterval(poll, 700);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [id]);
  if (!status) return null;
  return (
    <div className="recovery-progress">
      <p role="status">{status.message}</p>
      {status.newPid && (
        <p className="mono">
          PID {status.oldPid} → {status.newPid}
        </p>
      )}
      <button className="button" onClick={() => setShowOutput((v) => !v)}>
        {showOutput ? "Hide output" : "View Output"}
      </button>
      <button
        className="button"
        onClick={() => {
          void invoke<{ identity: { pid: number }; name: string }[]>(
            "recovery_inspect",
            { id },
          )
            .then((tree) =>
              setInspection(
                tree.map((p) => `PID ${p.identity.pid} · ${p.name}`).join("\n"),
              ),
            )
            .catch((e) => setInspection(String(e)));
        }}
      >
        Inspect Process
      </button>
      {inspection && (
        <pre className="code-block recovery-output">{inspection}</pre>
      )}
      {showOutput && (
        <pre className="code-block recovery-output">
          {status.output || "No output captured yet."}
        </pre>
      )}
    </div>
  );
}
export function LaunchSection({
  entry,
  copy,
}: {
  entry: PortEntry;
  copy: (text: string, label: string) => void;
}) {
  const launch = entry.launch;
  const [editing, setEditing] = useState(false);
  const [profile, setProfile] = useState<LaunchProfile | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [testId, setTestId] = useState<string>();
  const project = entry.project?.rootPath;
  const cwd = launch?.workingDirectory ?? entry.cwd;
  useEffect(() => {
    if (!project || !desktop) return;
    let disposed = false;
    void invoke<LaunchProfile | null>("recovery_profile", { project })
      .then((p) => {
        if (!disposed) setProfile(p);
      })
      .catch((e) => {
        if (!disposed) setMessage(String(e));
      });
    return () => {
      disposed = true;
    };
  }, [project]);
  async function terminal() {
    try {
      await invoke("recovery_terminal", { directory: cwd });
    } catch (e) {
      setMessage(String(e));
    }
  }
  async function save() {
    if (!profile) return;
    setBusy(true);
    try {
      await invoke("recovery_save_profile", { profile });
      setEditing(false);
      setMessage(
        "Recovery command saved. The port list will update on its next scan.",
      );
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="launch-section" aria-label="Launch context">
      <h4 className="section-label">LAUNCH</h4>
      {launch ? (
        <>
          <dl className="details">
            <dt>Command</dt>
            <dd className="mono">
              {launch.command}
              <button
                className="icon-button"
                title="Copy launch command"
                onClick={() => copy(launch.command, "Command")}
              >
                <Copy size={13} />
              </button>
            </dd>
            <dt>Directory</dt>
            <dd className="mono">
              {launch.workingDirectory}
              <button
                className="icon-button"
                title="Copy working directory"
                onClick={() =>
                  copy(launch.workingDirectory, "Working directory")
                }
              >
                <Copy size={13} />
              </button>
            </dd>
            <dt>Source</dt>
            <dd>{launchSources[launch.source]}</dd>
            <dt>Started</dt>
            <dd>
              {new Date(launch.launchRoot.startedAt * 1000).toLocaleString()}
            </dd>
            <dt>Launch root</dt>
            <dd className="mono">PID {launch.launchRoot.pid}</dd>
          </dl>
          {(launch.source === "parent_process" ||
            launch.source === "process_inspection") && (
            <p className="drawer-footnote">
              This may not exactly match the original command.
            </p>
          )}
          {launch.kind === "shell_command" && (
            <p className="drawer-footnote">
              Runs with {launch.shell}. Shell startup aliases and functions are
              not reconstructed.
            </p>
          )}
          {!entry.restartable && (
            <p className="drawer-footnote">{launch.reason}</p>
          )}
          <details>
            <summary>Environment ({launch.environment.length})</summary>
            <dl className="details recovery-environment">
              {launch.environment.map((v) => (
                <div className="detail-pair" key={v.name}>
                  <dt>{v.name}</dt>
                  <dd className="mono">{v.value ?? "••••••••"}</dd>
                </div>
              ))}
            </dl>
          </details>
          <RecoveryProgress id={launch.id} />
        </>
      ) : (
        <p className="drawer-footnote">
          Original launch command could not be determined. Enable shell
          integration in Conflict Autopilot to capture future launches.
        </p>
      )}
      <div className="recovery-actions">
        {cwd && (
          <button
            className="button"
            disabled={!desktop}
            onClick={() => void terminal()}
          >
            <Terminal size={14} />
            Open Terminal Here
          </button>
        )}
        {project && (
          <button
            className="button"
            disabled={!desktop}
            onClick={() => {
              setProfile(
                profile ?? {
                  projectId: project,
                  name: "Development",
                  executable: "",
                  args: [],
                  workingDirectory: cwd ?? project,
                },
              );
              setEditing((v) => !v);
            }}
          >
            {profile ? "Edit Recovery Command" : "Set Recovery Command"}
          </button>
        )}
      </div>
      {editing && profile && (
        <form
          className="recovery-recipe"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <label>
            Executable
            <input
              required
              placeholder="npm"
              value={profile.executable}
              onChange={(e) =>
                setProfile({ ...profile, executable: e.target.value })
              }
            />
          </label>
          <label>
            Arguments — one per line
            <textarea
              placeholder={"run\ndev"}
              value={profile.args.join("\n")}
              onChange={(e) =>
                setProfile({
                  ...profile,
                  args: e.target.value ? e.target.value.split("\n") : [],
                })
              }
            />
          </label>
          <label>
            Working directory
            <input
              required
              value={profile.workingDirectory}
              onChange={(e) =>
                setProfile({ ...profile, workingDirectory: e.target.value })
              }
            />
          </label>
          <p className="drawer-footnote">
            Uses Port Authority’s environment. Keep secrets in your project’s
            environment configuration. Arguments are passed literally.
          </p>
          <div className="recovery-actions">
            <button className="button primary" disabled={busy}>
              Save Recovery Command
            </button>
            <button
              type="button"
              className="button"
              onClick={() => setEditing(false)}
            >
              Cancel
            </button>
          </div>
        </form>
      )}
      {profile && !editing && (
        <button
          className="text-button"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void invoke<[string, RecoveryStatus]>("recovery_test_profile", {
              project,
              port: entry.port,
            })
              .then(([id, result]) => {
                setTestId(id);
                setMessage(result.message);
              })
              .catch((e) => setMessage(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          {busy ? "Testing…" : "Test Recovery Command"}
        </button>
      )}
      {testId && <RecoveryProgress id={testId} />}
      {message && (
        <p role="status" className="drawer-footnote">
          {message}
        </p>
      )}
    </section>
  );
}
