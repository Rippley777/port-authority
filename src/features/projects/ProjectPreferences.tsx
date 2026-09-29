import { useProjectContext } from "./useProjects";
import { editorNames, terminalNames } from "./types";
import type { Settings } from "../../lib/types";
export function ProjectPreferences({
  settings,
  change,
}: {
  settings: Settings;
  change: (s: Settings) => void;
}) {
  const { applications } = useProjectContext();
  return (
    <section className="panel settings-panel">
      <h3>Project applications</h3>
      <div className="setting-row">
        <div>
          <strong>Preferred Editor</strong>
          <p>
            Open a project root in your editor. Auto uses a detected
            installation.
          </p>
        </div>
        <select
          aria-label="Preferred Editor"
          value={settings.preferredEditor}
          onChange={(e) =>
            change({ ...settings, preferredEditor: e.target.value })
          }
        >
          {Object.entries(editorNames).map(([id, name]) => (
            <option key={id} value={id}>
              {id === "auto" ? "Auto detect" : name}
              {applications.editors.includes(id) ? " · detected" : ""}
            </option>
          ))}
        </select>
      </div>
      {settings.preferredEditor === "custom" && (
        <div className="setting-row">
          <div>
            <strong>Custom editor executable</strong>
            <p>
              Absolute executable path. The project directory is passed as one
              argument.
            </p>
          </div>
          <input
            className="custom-editor-input"
            aria-label="Custom editor executable"
            value={settings.customEditor}
            placeholder="/absolute/path/to/editor"
            onChange={(e) =>
              change({ ...settings, customEditor: e.target.value })
            }
          />
        </div>
      )}
      <div className="setting-row">
        <div>
          <strong>Preferred Terminal</strong>
          <p>Open a new terminal at the project directory.</p>
        </div>
        <select
          aria-label="Preferred Terminal"
          value={settings.preferredTerminal}
          onChange={(e) =>
            change({ ...settings, preferredTerminal: e.target.value })
          }
        >
          {Object.entries(terminalNames)
            .filter(
              ([id]) =>
                id === "auto" ||
                !applications.terminals.length ||
                applications.terminals.includes(id) ||
                id === settings.preferredTerminal,
            )
            .map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
        </select>
      </div>
    </section>
  );
}
