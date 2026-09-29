import { FolderGit2 } from "lucide-react";
import type { PortEntry } from "../../lib/types";
import { useProjectContext } from "./useProjects";
export function ProjectBadge({
  entry,
  compact = false,
}: {
  entry: PortEntry;
  compact?: boolean;
}) {
  const { open } = useProjectContext();
  const p = entry.project;
  if (!p)
    return (
      <span className="project-identity">
        <strong>{entry.serviceName ?? entry.process}</strong>
        {!compact && <small>{entry.process}</small>}
      </span>
    );
  return (
    <button
      className={`project-badge ${compact ? "compact" : ""}`}
      title={`${p.displayPath}\n${p.evidence}`}
      onClick={(e) => {
        e.stopPropagation();
        open(p);
      }}
    >
      <FolderGit2 size={16} />
      <span>
        <strong>
          {p.name}
          {p.confidence === "LOW" && <em> · inferred</em>}
        </strong>
        {!compact && (
          <small>
            {entry.serviceName ?? p.projectType} · {entry.process}
          </small>
        )}
      </span>
    </button>
  );
}
