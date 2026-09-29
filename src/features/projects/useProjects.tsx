import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop, writeClipboard } from "../../lib/api";
import type { PortEntry, Settings } from "../../lib/types";
import {
  editorNames,
  type ProjectAction,
  type ProjectIdentity,
  type RecentProject,
} from "./types";
interface Snapshot {
  projects: RecentProject[];
  resolving: boolean;
  storageError: string | null;
}
interface Context {
  projects: RecentProject[];
  ports: PortEntry[];
  resolving: boolean;
  error: string | null;
  selected: ProjectIdentity | null;
  open: (p: ProjectIdentity) => void;
  close: () => void;
  action: (p: ProjectIdentity, a: ProjectAction) => Promise<void>;
  pin: (p: ProjectIdentity, pinned: boolean) => Promise<void>;
  forget: (p: ProjectIdentity) => Promise<void>;
  refresh: (project?: ProjectIdentity) => Promise<void>;
  editorName: string;
  showPorts: (p: ProjectIdentity) => void;
  applications: { editors: string[]; terminals: string[] };
}
export const ProjectsContext = createContext<Context | null>(null);
export const useProjectContext = () => {
  const context = useContext(ProjectsContext);
  if (!context) throw new Error("Missing project context");
  return context;
};
function previewRecent(): RecentProject[] {
  try {
    const items = JSON.parse(
      localStorage.getItem("pa-preview-projects") ?? "[]",
    ) as RecentProject[];
    return Array.isArray(items)
      ? items
          .filter(
            (p) =>
              typeof p?.identity?.rootPath === "string" &&
              typeof p.identity.name === "string" &&
              typeof p.identity.displayPath === "string" &&
              Array.isArray(p.identity.frameworks) &&
              p.identity.frameworks.every((f) => typeof f === "string") &&
              Array.isArray(p.identity.manifests) &&
              typeof p.lastObserved === "number" &&
              Number.isFinite(p.lastObserved) &&
              typeof p.pinned === "boolean" &&
              Array.isArray(p.knownPorts) &&
              p.knownPorts.every(
                (n) => Number.isInteger(n) && n > 0 && n <= 65535,
              ),
          )
          .slice(0, 200)
      : [];
  } catch {
    return [];
  }
}
export function useProjects(
  ports: PortEntry[],
  settings: Settings,
  notify: (message: string, error?: boolean) => void,
  refreshPorts: () => Promise<void>,
  showPorts: (p: ProjectIdentity) => void,
): Context {
  const [snapshot, setSnapshot] = useState<Snapshot>({
    projects: desktop ? [] : previewRecent(),
    resolving: false,
    storageError: null,
  });
  const [selected, setSelected] = useState<ProjectIdentity | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [applications, setApplications] = useState({
    editors: [] as string[],
    terminals: [] as string[],
  });
  const close = useCallback(() => setSelected(null), []);
  const poll = useCallback(async () => {
    if (!desktop) return;
    try {
      setSnapshot(await invoke<Snapshot>("projects_snapshot"));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);
  useEffect(() => {
    void poll();
    if (!desktop) return;
    const timer = setInterval(() => void poll(), 1500);
    void invoke<typeof applications>("project_applications")
      .then(setApplications)
      .catch(() => {});
    return () => clearInterval(timer);
  }, [poll]);
  useEffect(() => {
    if (desktop) return;
    setSnapshot((old) => {
      const recent = new Map(old.projects.map((p) => [p.identity.rootPath, p]));
      for (const p of ports) {
        if (!p.project || p.project.confidence === "LOW") continue;
        const previous = recent.get(p.project.rootPath);
        recent.set(p.project.rootPath, {
          identity: p.project,
          lastObserved: Math.floor(Date.now() / 1000),
          pinned: previous?.pinned ?? false,
          knownPorts: [
            ...new Set([...(previous?.knownPorts ?? []), p.port]),
          ].sort((a, b) => a - b),
        });
      }
      return { ...old, projects: [...recent.values()] };
    });
  }, [ports]);
  useEffect(() => {
    if (!desktop)
      try {
        localStorage.setItem(
          "pa-preview-projects",
          JSON.stringify(snapshot.projects),
        );
      } catch {
        /* preview storage optional */
      }
  }, [snapshot.projects]);
  const editorId =
    settings.preferredEditor === "auto"
      ? (applications.editors[0] ?? "auto")
      : settings.preferredEditor;
  async function action(p: ProjectIdentity, a: ProjectAction) {
    try {
      if (a === "copy") {
        await writeClipboard(p.rootPath);
        notify("Project path copied to clipboard");
        return;
      }
      if (!desktop) {
        notify("Preview only. Project applications open in the desktop app.");
        return;
      }
      await invoke("project_action", {
        root: p.rootPath,
        action: a,
        preference:
          a === "editor"
            ? settings.preferredEditor
            : settings.preferredTerminal,
        custom: settings.customEditor || null,
      });
      notify(
        a === "editor"
          ? `Opened ${p.name} in ${editorNames[editorId]}`
          : a === "terminal"
            ? `Opened terminal for ${p.name}`
            : a === "repository"
              ? "Opened repository"
              : "Revealed project directory",
      );
    } catch (e) {
      notify(String(e), true);
    }
  }
  async function pin(p: ProjectIdentity, pinned: boolean) {
    try {
      if (desktop) {
        await invoke("project_pin", { root: p.rootPath, pinned });
        await poll();
      } else
        setSnapshot((old) => ({
          ...old,
          projects: old.projects.map((r) =>
            r.identity.rootPath === p.rootPath ? { ...r, pinned } : r,
          ),
        }));
    } catch (e) {
      notify(String(e), true);
    }
  }
  async function forget(p: ProjectIdentity) {
    try {
      if (desktop) {
        await invoke("project_forget", { root: p.rootPath });
        await poll();
      } else
        setSnapshot((old) => ({
          ...old,
          projects: old.projects.filter(
            (r) => r.identity.rootPath !== p.rootPath,
          ),
        }));
      close();
    } catch (e) {
      notify(String(e), true);
    }
  }
  async function refresh(project?: ProjectIdentity) {
    try {
      if (desktop && project) {
        const updated = await invoke<ProjectIdentity>("project_refresh", {
          root: project.rootPath,
        });
        setSelected((current) =>
          current?.rootPath === updated.rootPath ? updated : current,
        );
        await poll();
        return;
      }
      if (desktop) await invoke("projects_refresh");
      await refreshPorts();
      await poll();
    } catch (e) {
      notify(String(e), true);
    }
  }
  return {
    ...snapshot,
    error: error ?? snapshot.storageError,
    ports,
    selected,
    open: setSelected,
    close,
    action,
    pin,
    forget,
    refresh,
    editorName: editorNames[editorId] ?? "Editor",
    showPorts,
    applications,
  };
}
