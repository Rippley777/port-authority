import { useState } from "react";
import {
  FolderGit2,
  GitBranch,
  Pin,
  Search,
  RefreshCw,
  ArrowRight,
  X,
  Copy,
} from "lucide-react";
import { Modal } from "../../components/Modal";
import { useProjectContext } from "./useProjects";
import { ProjectActions } from "./ProjectActions";
import { projectSearch, type ProjectIdentity } from "./types";
import { uptime } from "../../lib/ports";
import type { PortEntry } from "../../lib/types";
function services(ports: PortEntry[], root: string) {
  return ports.filter(
    (p) => p.project?.rootPath === root && p.project.confidence !== "LOW",
  );
}
function uniqueServices(ports: PortEntry[]) {
  return [
    ...new Map(ports.map((p) => [`${p.pid}:${p.startedAt}`, p])).values(),
  ];
}
export function ProjectsScreen({
  inspect,
}: {
  inspect: (p: PortEntry) => void;
}) {
  const context = useProjectContext();
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState("All projects");
  const list = context.projects
    .filter(
      (r) =>
        projectSearch(r.identity).includes(query.toLowerCase()) &&
        (tab !== "Pinned" || r.pinned) &&
        (tab !== "Running" ||
          services(context.ports, r.identity.rootPath).length),
    )
    .sort(
      (a, b) =>
        Number(b.pinned) - Number(a.pinned) ||
        Number(services(context.ports, b.identity.rootPath).length > 0) -
          Number(services(context.ports, a.identity.rootPath).length > 0) ||
        b.lastObserved - a.lastObserved,
    );
  return (
    <div className="projects-workspace">
      <section className="panel">
        <div className="project-toolbar">
          <div className="project-search">
            <Search size={16} />
            <input
              aria-label="Search projects"
              placeholder="Search projects, repositories, or paths…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <button className="button" onClick={() => void context.refresh()}>
            <RefreshCw size={14} className={context.resolving ? "spin" : ""} />
            Refresh metadata
          </button>
        </div>
        <div className="project-tabs">
          {["All projects", "Running", "Pinned"].map((t) => (
            <button
              key={t}
              className={tab === t ? "active" : ""}
              onClick={() => setTab(t)}
            >
              {t}
            </button>
          ))}
          <span>{context.projects.length} discovered locally</span>
        </div>
      </section>
      {context.error && (
        <p className="error-banner" role="alert">
          {context.error}
        </p>
      )}
      <div className="project-list">
        {list.map((r) => {
          const p = r.identity;
          const entries = services(context.ports, p.rootPath);
          return (
            <article className="panel project-card" key={p.rootPath}>
              <div className="project-card-heading">
                <span className="project-folder">
                  <FolderGit2 size={21} />
                </span>
                <button
                  className="project-title"
                  onClick={() => context.open(p)}
                >
                  <strong>{p.name}</strong>
                  <small>{p.displayPath}</small>
                </button>
                <span
                  className={`project-running ${entries.length ? "active" : ""}`}
                >
                  <span className="status-dot" />
                  {entries.length ? "Running" : "Not running"}
                </span>
                <button
                  className={`icon-button ${r.pinned ? "pinned" : ""}`}
                  aria-label={`${r.pinned ? "Unpin" : "Pin"} ${p.name}`}
                  onClick={() => void context.pin(p, !r.pinned)}
                >
                  <Pin size={16} />
                </button>
              </div>
              <div className="project-meta">
                <span>
                  <GitBranch size={13} />
                  {p.gitBranch ?? "No Git branch"}
                  {p.gitDirty === true && " · modified"}
                </span>
                <span>
                  {p.frameworks.join(" / ") || p.projectType || "Unknown type"}
                </span>
                <span>
                  {p.repository
                    ? [p.repository.owner, p.repository.repository]
                        .filter(Boolean)
                        .join("/")
                    : "Local project"}
                </span>
              </div>
              <div className="project-services">
                {uniqueServices(entries).map((e) => (
                  <button
                    key={`${e.pid}:${e.startedAt}`}
                    onClick={() => inspect(e)}
                  >
                    <span className="status-dot" />
                    <strong>{e.serviceName ?? e.process}</strong>
                    <span className="mono">
                      {[
                        ...new Set(
                          entries
                            .filter(
                              (x) =>
                                x.pid === e.pid && x.startedAt === e.startedAt,
                            )
                            .map((x) => `:${x.port}`),
                        ),
                      ].join("  ")}
                    </span>
                    <span className="muted mono">PID {e.pid}</span>
                    <ArrowRight size={14} />
                  </button>
                ))}
                {!entries.length && (
                  <div className="project-idle">
                    <span>Last active {uptime(r.lastObserved)} ago</span>
                    <span className="mono">
                      Known ports{" "}
                      {r.knownPorts.map((n) => `:${n}`).join("  ") || "—"}
                    </span>
                  </div>
                )}
              </div>
              <ProjectActions project={p} />
            </article>
          );
        })}
      </div>
      {!list.length && (
        <section className="panel empty-state roomy">
          <FolderGit2 size={30} />
          <h3>
            {query
              ? "No matching projects"
              : tab === "Pinned"
                ? "Keep a project close."
                : "Your projects will appear here."}
          </h3>
          <p>
            {query
              ? "Try a repository name or project path."
              : "Start a local development service. Port Authority will resolve its project from process metadata."}
          </p>
        </section>
      )}
      <p className="project-caption">
        Only observed projects appear here. Git metadata is cached; nothing is
        uploaded.
      </p>
    </div>
  );
}
export function ProjectDetails({
  inspect,
}: {
  inspect: (p: PortEntry) => void;
}) {
  const c = useProjectContext();
  const p = c.selected;
  if (!p) return null;
  const recent = c.projects.find((r) => r.identity.rootPath === p.rootPath);
  const entries = services(c.ports, p.rootPath);
  const current = recent?.identity ?? p;
  return (
    <Modal
      label={`Project details for ${p.name}`}
      close={c.close}
      className="drawer-backdrop project-details"
    >
      <div className="drawer-top">
        <span>PROJECT DETAILS</span>
        <button
          className="icon-button"
          aria-label="Close project details"
          onClick={c.close}
        >
          <X size={19} />
        </button>
      </div>
      <div className="drawer-body">
        <div className="project-detail-title">
          <FolderGit2 size={28} />
          <div>
            <h2>{current.name}</h2>
            <p>
              {current.frameworks.join(" / ") ||
                current.projectType ||
                "Local project"}
            </p>
          </div>
          {recent && (
            <button
              className={`icon-button ${recent.pinned ? "pinned" : ""}`}
              aria-label={`${recent.pinned ? "Unpin" : "Pin"} ${p.name}`}
              onClick={() => void c.pin(p, !recent.pinned)}
            >
              <Pin size={18} />
            </button>
          )}
        </div>
        <button
          className="project-path"
          onClick={() => void c.action(p, "copy")}
          title={p.rootPath}
        >
          <span>{p.displayPath}</span>
          <Copy size={14} />
        </button>
        <div className="project-detail-stats">
          <div>
            <strong>{uniqueServices(entries).length}</strong>
            <span>Processes</span>
          </div>
          <div>
            <strong>{new Set(entries.map((p) => p.port)).size}</strong>
            <span>Ports</span>
          </div>
          <div>
            <strong className={entries.length ? "green-text" : "muted"}>
              {entries.length ? "Running" : "Not running"}
            </strong>
            <span>Observed status</span>
          </div>
        </div>
        <ProjectActions project={current} />
        <button
          className="text-button project-refresh-details"
          onClick={() => void c.refresh(current)}
        >
          <RefreshCw size={12} />
          Refresh project details
        </button>
        <dl className="project-metadata">
          <dt>Repository</dt>
          <dd>
            {current.repository
              ? [current.repository.owner, current.repository.repository]
                  .filter(Boolean)
                  .join("/")
              : "No remote detected"}
          </dd>
          <dt>Branch</dt>
          <dd>{current.gitBranch ?? "Unavailable"}</dd>
          <dt>Tracked Git changes</dt>
          <dd>
            {current.gitDirty === null
              ? "Unavailable"
              : current.gitDirty
                ? "Modified"
                : "Clean"}
          </dd>
          <dt>Manifests</dt>
          <dd>{current.manifests.join(", ") || "Git repository"}</dd>
          <dt>Attribution</dt>
          <dd>
            {p.evidence}
            {p.confidence !== "HIGH" && " · inferred"}
          </dd>
        </dl>
        <div className="section-label">RUNNING SERVICES</div>
        <div className="project-services details-services">
          {entries.map((e) => (
            <button
              key={e.id}
              onClick={() => {
                c.close();
                inspect(e);
              }}
            >
              <span className="status-dot" />
              <strong>{e.serviceName ?? e.process}</strong>
              <span className="mono">:{e.port}</span>
              <span className="muted">{e.protocol}</span>
              <ArrowRight size={14} />
            </button>
          ))}
        </div>
        {!entries.length && (
          <p className="muted">
            No listening processes currently observed. Known ports:{" "}
            {recent?.knownPorts.join(", ") || "—"}.
          </p>
        )}
        <button
          className="button project-show-ports"
          onClick={() => {
            c.showPorts(p);
            c.close();
          }}
        >
          Show project ports <ArrowRight size={14} />
        </button>
        {recent && !entries.length && (
          <button
            className="text-button project-forget"
            onClick={() => void c.forget(p)}
          >
            Forget recent project
          </button>
        )}
      </div>
    </Modal>
  );
}
export function ProjectSection({ project }: { project: ProjectIdentity }) {
  const c = useProjectContext();
  return (
    <section className="drawer-project">
      <button className="project-title" onClick={() => c.open(project)}>
        <strong>
          <FolderGit2 size={15} />
          {project.name}
        </strong>
        <small>{project.displayPath}</small>
      </button>
      <div className="project-meta">
        <GitBranch size={13} />
        {project.gitBranch ?? "Local project"}
      </div>
      <ProjectActions project={project} />
    </section>
  );
}
