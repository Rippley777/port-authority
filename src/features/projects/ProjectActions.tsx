import { Code2, Terminal, ExternalLink, FolderOpen, Copy } from "lucide-react";
import type { ProjectIdentity } from "./types";
import { useProjectContext } from "./useProjects";
export function ProjectActions({
  project: p,
  menu = false,
  done,
  omitTerminal = false,
}: {
  project: ProjectIdentity;
  menu?: boolean;
  done?: () => void;
  omitTerminal?: boolean;
}) {
  const { action, editorName } = useProjectContext();
  const actions = [
    { id: "editor" as const, label: `Open in ${editorName}`, icon: Code2 },
    { id: "terminal" as const, label: "Open Terminal Here", icon: Terminal },
    ...(p.repository?.webUrl
      ? [
          {
            id: "repository" as const,
            label: "Open Repository",
            icon: ExternalLink,
          },
        ]
      : []),
    { id: "reveal" as const, label: "Reveal Directory", icon: FolderOpen },
    { id: "copy" as const, label: "Copy Project Path", icon: Copy },
  ];
  return (
    <div className={menu ? "project-menu-actions" : "project-actions"}>
      {actions
        .filter((a) => !omitTerminal || a.id !== "terminal")
        .map((a) => (
          <button
            key={a.id}
            title={a.label}
            role={menu ? "menuitem" : undefined}
            className={menu ? "" : "button"}
            onClick={() => {
              void action(p, a.id);
              done?.();
            }}
          >
            <a.icon size={14} />
            {a.label}
          </button>
        ))}
    </div>
  );
}
