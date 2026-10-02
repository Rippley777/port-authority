# Project Awareness

Port Authority gives local services a project identity while preserving the underlying process, PID, executable, arguments, and working directory. Project resolution is local and independent of process-control authorization.

## Use it

Start a development service and open **Ports**. A listener appears immediately; its project name and service type arrive asynchronously. Click the project name for its root, frameworks, repository, branch, tracked Git status, running processes, and ports. Right-click a row for grouped project actions above process actions.

**Projects** groups observed listeners by canonical project root. Pin a project to keep it visible, including when it has no listeners. Recent projects retain their last observed time and known ports. An idle project's details offer **Forget recent project**. Restarting a service discovers its project again. There is no filesystem-wide project scan.

Search the Ports view, Projects view, or command palette using a name, repository owner/name, absolute root, or displayed `~/…` path. The palette includes editor, terminal, repository, reveal, and show-ports commands for recent projects. Port Timeline snapshots project metadata through the cached native resolver when an ownership transition is recorded. Historical snapshots stay attached to that process identity after it exits. Live late enrichment still validates PID, start time, and metadata before updating a port row.

In **Settings → Project applications**, choose Auto detect or a preferred editor and terminal. A missing application produces a useful error. The browser preview demonstrates grouping and pinning with separate sample data; application-launch buttons explicitly report that native desktop integration is required.

## Resolution rules

The shared Rust subsystem lives in `src-tauri/src/projects/`:

- `resolver.rs`: process evidence, upward root discovery, display identity, service recognition.
- `manifests.rs`: bounded metadata reads and manifest names/dependencies.
- `git.rs`, `repository.rs`: local Git metadata, worktrees, remote parsing and safe web links.
- `models.rs`: UI-independent project, repository, confidence, and recent-project models.
- `cache.rs`: asynchronous enrichment, stable process keys, shared root metadata, recent storage.
- `actions.rs`: structured native application execution.

Resolution requires an accessible process working directory. Missing or protected cwd remains unknown; executable, command argument and parent paths are not filesystem fallbacks. System-owned listeners remain unattributed. No framework or project is inferred from a port number.

The nearest directory with a known manifest or `.git` wins. `repo/apps/web/package.json` therefore identifies the web application, while repository metadata can still come from the enclosing Git repository. Dependency directories (`node_modules`, `target`, `.venv`, `vendor`) are skipped so their manifests do not masquerade as the application. Searches canonicalize paths, walk at most 24 ancestors, and stop before the home directory, filesystem root, or a device boundary. There is no directory enumeration. .NET discovery checks `<directory-name>.sln` and `<directory-name>.csproj`. Protected macOS paths and untrusted symlinks are excluded before filesystem access.

Markers include package.json, Cargo.toml, pyproject.toml, requirements.txt, go.mod, pom.xml, build.gradle/build.gradle.kts, composer.json, Gemfile, Compose YAML names, solution and C# project files. Names are extracted from Node, Cargo, Python/Poetry, Go, and Composer manifests where available, otherwise from the root directory. Package scopes are omitted from display names and ordinary hyphens/underscores become spaces. Dependency and command evidence identify common Node and Python frameworks. Other markers establish the ecosystem; this is deliberately not a complete build-system parser.

Working-directory evidence has high confidence. Older persisted parent/argument inferences retain their original confidence labels, but new discovery does not probe those paths. Attribution cannot grant process-control permissions. Project details expose the attribution reason, without percentages or numerical risk scores.

Docker Compose files identify local project roots. Container labels, port mappings, and Docker daemon inspection are **not implemented**. The resolver's candidate/identity boundary is the extension point for a future Docker metadata provider. `com.docker.backend` is not automatically assigned to a project.

## Git and remote metadata

Git is optional: manifest-based identity works when Git is absent or metadata cannot be read. Local Git commands handle `.git` files and worktrees, prefer `origin` among multiple remotes, and fall back deterministically to another parseable remote. Detached HEAD displays a short commit identifier. Dirty/clean status covers **tracked changes only**; untracked files and submodules are not scanned.

SSH/scp-style and HTTPS remotes expose provider, owner/group, repository, and a sanitized remote identifier. Credentials, queries, and fragments are discarded. GitHub, GitLab, and Bitbucket SSH remotes can become HTTPS links; self-hosted HTTPS remotes can expose a safe web link. Unknown SSH hosts and ambiguous/local remote paths do not get a guessed browser action. A browser opens only after an explicit user action.

Each Git subprocess has a 1.2-second deadline and a 64 KiB output limit. Optional locks and fsmonitor are disabled. Repository clean/process filters are disabled when checking tracked status, so discovery does not run those project-defined commands. No fetch, network lookup, checkout, hook, or dependency installation occurs.

## Performance and identity safety

The socket scan only reads cached enrichment and submits its snapshot to one background worker. New snapshots replace queued work instead of accumulating. When resolution completes, a `projects-resolved` Tauri event updates current rows, including after a manual scan while monitoring is paused. Frontend event merging checks the socket row, PID, and process start time before applying optional metadata.

Process cache keys include PID **and start time**, cwd, executable, arguments, and parent PID. Associations are pruned when listeners disappear; missing start times are not given cached project identities. Process results (including unsuccessful lookups) remain cached until identity/evidence changes or an explicit refresh. Shared root metadata has a 60-second cache for new associations. **Refresh metadata** invalidates both caches and scans again. Project details also offer a targeted refresh for an idle recent project, without changing its last-active time. Removed or inaccessible paths fall back safely without guessing.

Conflict Autopilot uses the same root resolver for cross-project safety decisions and the same cached identities for owner/listener presentation. It offers **Open [project]** beside the original guarded recovery actions. Enrichment never modifies its PID/start-time checks, classification requirements, confirmations, or termination policy. An unresolved owner keeps the existing conservative fallback.

## Local persistence and privacy

The native app stores `recent-projects.json` in its Tauri application-data directory. It contains only identity/repository metadata, last-observed timestamps, pin state, and known port numbers. Recent history is capped at 200 entries, with pinned projects retained first; known ports are capped at 128 per project. Updates use a temporary file and atomic rename, private file permissions on Unix, and timestamp-only writes are limited to once a minute. Errors appear on the Projects screen.

Manifests are limited to 256 KiB, parsed as metadata, and never retained as file contents. Nonregular files and symlinks are not read as manifests. Source files, command environments, and full process command lines are not stored in recent-project history. Settings/preferences remain in local webview storage. No repository information, Git remotes, paths, or analytics are uploaded.

## Native application actions

Project actions accept only an observed/recent canonical root, recheck that it exists and has not changed through a symlink, and execute explicit argument vectors. Paths containing spaces, quotes, dollar signs, or semicolons remain literal arguments. Custom editors use an absolute executable path (an `.exe` on Windows), with the root passed as one argument; arbitrary shell command templates are not accepted.

- **macOS:** app detection in `/Applications` and `~/Applications`, or editor CLI detection on PATH. Launch Services opens editors, Terminal/iTerm/Ghostty directories, and Finder reveal. Warp uses its [documented new-window URI](https://docs.warp.dev/terminal/more-features/uri-scheme), with the path URL-encoded. Terminal application preferences may affect window/tab behavior; GUI integrations beyond the installed tools require platform testing.
- **Linux:** editor CLI detection on PATH; system terminal adapters and folder opening through the platform opener. Explicit working-directory arguments are used where supported, with process cwd for other terminal launchers.
- **Windows:** editor executable detection on PATH and common installation directories; Windows Terminal with `-d` or PowerShell with cwd; Explorer folder opening. macOS/Linux-only choices return an explicit unsupported preference error.

VS Code, VS Code Insiders, Cursor, Zed, Sublime Text, IntelliJ IDEA, WebStorm, PyCharm, and a custom executable are recognized where installed. This workspace verifies macOS compilation, resolver behavior, safe custom-editor invocation, and packaging. Linux and Windows remain covered by the existing CI matrix; their GUI application launch behavior was not exercised locally.

## Verification

Native tests cover upward/nested discovery, monorepos, dependency directories, Cargo/Python names, malformed/oversized/unreadable/removed manifests, absent cwd, parent attribution, infrastructure exclusions, remote formats, multiple remotes, worktrees, tracked Git changes, disabled Git filters, PID reuse, cache reuse/invalidation, pruning, pin persistence, literal launch arguments, and asynchronous enrichment of a real listening socket.

Browser tests cover project search by name/remote/path, details and show-ports navigation, grouped services, infrastructure exclusion, pin persistence, idle/recent projects, editor preferences, clipboard/context actions, command-palette integration, Autopilot project navigation, and a narrow viewport. Existing process-control and Autopilot tests remain part of the verification suite.

See [macOS process privacy](macos-process-privacy.md) for the live root-cause evidence and denied-access policy.
