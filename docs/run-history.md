# Run History and Favorite Launch Points

Run History remembers commands independently of their processes. Port Timeline records socket ownership; a history run points to a reusable launch context and its observed process/ports.

## Diagnosis of the original bugs

1. Display commands came from shell captures, recovered process argv, or explicit project recipes. `launch_context::from_capture` formatted that argv for History, Favorites, and Timeline.
2. `launch_context::redact` collected **every environment value outside a small public allowlist** and globally replaced it in command text. `safe_argv` reused that redactor.
3. Values such as `6`, `bin`, `node`, and ordinary NVM paths consequently masked unrelated versions, options, and paths. Substring matching on names such as `KEY` also overclassified variables.
4. SQLite retained sanitized `launch_contexts`, `command_runs`, `run_ports`, and `favorite_commands`. The original executable/argv/cwd/environment lived only in a RAM `Capture`. Every saved display context was marked unrecoverable.
5. Run Again already spawned a structured RAM capture through the detached supervisor; it did not execute React's command string. It had no durable execution record to reload.
6. An app restart or eviction of a stopped RAM record lost that capture, producing “Command unavailable.” Repeated scans could also replace a run's launch root with its listener child, creating duplicate executions. Persisted RUNNING rows could remain active after the process had exited.

## Execution and display are separate

```text
Captured executable + literal argv + cwd
  -> private LaunchContext in execution_contexts (schema 2)
  -> native structured execution through the existing supervisor

Capture
  -> argument-aware sanitizer
  -> DisplayLaunchContext in launch_contexts
  -> History / Favorites / Timeline
```

`execution::LaunchContext` is never returned through Tauri. It stores the absolute executable, argv[0], exact argument boundaries, cwd, execution type, shell, source, capture time, project reference, environment strategy, and (on Unix) directory identity. The display DTO has a different Rust and TypeScript type. Display text is never parsed or promoted into executable arguments. Execution rejects masking placeholders.

Execution and display records are saved in one SQLite transaction. Process exit and run retention do not delete execution contexts or favorite links. Evicted records can be loaded by their context ID. Favorites store IDs, not commands. Each Timeline launch snapshot carries the specific `runId`, alongside `relaunchedAt`.

Fingerprints group project, executable, arguments, cwd, execution type, and shell independently of PID. Secret-bearing command variants use separate context IDs so identical masks cannot conflate different commands. Individual runs retain their launch-root identity even when a child owns the port. Background completion is scoped to the recorded launch root, so a stale second app instance cannot repeatedly end a newer run. Runs started within the same second retain stable creation order. An exited root ends stale RUNNING/UNVERIFIED records at the time exit is detected, explicitly noting when the exact exit time was not observed.

## Redaction

The sanitizer classifies sensitive variable/option names and masks their values. It handles assignments, `--token value`, `--password=value`, quoted shell/JSON assignments, URL credentials, and sensitive URL query parameters. NVM paths, development commands, versions, project names, and ordinary arguments remain readable.

It does not substitute arbitrary environment substrings into argv. An entire argument matching a long, explicitly named credential can be masked; unstructured output additionally masks whole credential tokens at boundaries. Short values such as `6` and `bin` never rewrite paths or versions. The canonical formatter handles quoting once, after argument sanitization.

## Environment and shell policy

During the current app session, an observed capture retains its original environment privately in memory. After restart, the explicit strategy is **current environment with captured toolchain paths**. Only the allowlisted environment keys in `launch_context::public_env` are retained for execution: PATH, NVM/virtualenv/Cargo/Rust toolchain locations, HOME, locale, and selected runtime settings such as NODE_ENV and PORT. Secret values and full environment snapshots are not persisted. Runtime credentials must come from the current app environment or the project's normal configuration.

Arbitrary one-off exported variables, shell functions, aliases, activation state, and terminal-only secrets cannot be restored from history. Put required setup in project configuration or explicitly capture a shell invocation, for example `pa /bin/zsh -lc 'source .venv/bin/activate && python app.py'`. `pa` captures external executable arguments, not arbitrary interactive shell syntax. Direct commands stay direct; explicit shell invocations retain their shell and literal script argument.

If argument sanitization detects secrets, the original arguments remain RAM-only. They can be rerun during that session, but after restart the entry explains that secret arguments were not saved. Masked placeholders are never substituted for missing secrets.

## Evidence and availability

Port Authority relaunches take precedence, then observed terminal launches, recovered launch roots/parents, and intact direct process arguments. Project recipes fill unavailable contexts; they cannot replace a stronger captured command. Recovery confidence is separately represented as Exact, Observed, Recovered, Inferred, or Unavailable. This implementation does not infer a package manager from a manifest; unknown runtime titles remain unavailable. Inferred is reserved for an explicit, confirmed future suggestion.

On macOS, fallback uses only cached optional process metadata, requires intact supported argv and a compatible executable, and uses the current app environment. It never requests other processes' environments. Rewritten `npm run dev` titles attached to a Node binary are not treated as executable npm argv. Shell capture remains the strongest way to retain the original human launch command.

Captured launch cwd takes precedence for project identity. Runtime installations and caches (`.nvm`, `.volta`, `.asdf`, `.rustup`, `.cache`, `_npx`, `.next`) do not become projects. Dependency directories resolve to the owning project above them. A real manifest-rooted project deliberately located under a temporary directory is still permitted when there is no stronger location; a bare temporary runtime directory is not a project.

Legacy records contain only display data. They remain visible as **Legacy history entry: original launch command was not retained**, and cannot execute. A new observation or explicit project recipe supplies fresh trustworthy data; there is no attempt to reconstruct originals from bullets.

## Preflight and verification

Every port timeline event has a Play / Run Again button, including releases (which use the released owner's launch ID). Ownership sessions also provide the action. Clicking resolves the current saved launch context rather than trusting a snapshot's old availability flag, then uses the same confirmation, duplicate checks, port checks, and execution path as Run History. Feedback and captured output appear on the item. Records without a saved launch ID show a disabled button explaining that the original command was not saved.

Run Again checks the original cwd, directory identity, executable, project association, complete structured capture, equivalent active commands, and expected-port conflicts. Unknown adapters require review; standard npm/pnpm/yarn/bun, Python, cargo run, and cargo tauri dev launches preserve their package manager and arguments.

The detached supervisor receives the private capture through stdin and uses `Command::new(...).args(...).current_dir(...)`. Readiness uses explicit port arguments, simple Next/Vite package-script port flags, or PORT, then ports stable across prior runs when available. Full observed-port history still includes temporary worker sockets; they do not override explicit service configuration. Success requires the root to remain alive and expected service ports to belong to its tree across three scans. A launch with no historical ports is Unverified, and does not claim service readiness. Launch failures retain inspectable output. Duplicate launches and foreign port owners are rejected before spawning.

Recently Run prioritizes recoverable stopped/completed jobs, recoverable failures, then active project commands. Runtime installations and unrecoverable entries remain out of that section. The full history remains available.

## Development diagnostics and validation

History Details has an on-demand diagnostics view in development. The backend rejects diagnostics requests in release builds. It exposes project/source/confidence, executable, argument count, cwd, display command, argument classification indices, environment key classifications, environment strategy, and an availability reason. It never exposes raw secret arguments or environment values.

Regression tests cover readable commands/paths/NVM/version text, short environment values, masked secrets, exact npm/pnpm/cargo/shell argument vectors, SQLite privacy, legacy entries, moved/replaced cwd, app restart, cache eviction, duplicate prevention, new run IDs, pinned commands, and PID reuse. Native service tests use the actual executor and port scanner; browser tests cover History/Favorites actions.

An ignored native dogfood test can also target a real project without replacing its history database. Create a temporary JSON spec with `cwd`, `argv` (an array), and `port` (optionally `expectedProcess` to wait for a desktop child, or `database` for a dedicated test app), then run the compiled Rust test binary with `PA_DOGFOOD_SPEC=/absolute/spec.json --ignored --exact recovery::tests::real_project_history_dogfood --nocapture`. It refuses an occupied port, launches the selected command, verifies project and port association, stops only its captured process tree, reopens the database, calls the same Run Again backend as the UI, and checks the new run and expected port. Run the test binary directly when testing a Cargo/Tauri project to avoid holding Cargo's build lock during the test.

### Local dogfood results (2026-10-01)

- Real Rippley Labs `npm run dev`: stopped, reopened history, relaunched from exact structured data, verified project ownership and :3077.
- Real Port Authority `npm run desktop -- --config <isolated config>`: verified both Vite :1431 and the desktop process before and after restarting from reopened history. The test used a separate app identifier and left the existing :1420 server untouched.
- Native History click-through in **Port Authority History Check**: readable `npm run dev`, correct cwd, Run Again success message verifying :3077, one new run after repeated scans, and Details showing exact recovery confidence, environment policy, and a new run ID. This check caught and fixed transient Next.js worker-port readiness and stale-instance run-completion bugs.

## Restart a running service

Use Restart directly from Ports and Processes rows, process details, the port context menu, Favorites, Run History cards and details, or Port Timeline events and ownership history. Each action confirms the live launch identity and the ports it must reopen. History matches the saved launch ID or command fingerprint, so a different command in the same project is never selected just because it occupies an old port.

Stopped commands remain available through Run Again in Favorites, Run History, the command palette, and Port Timeline. The original command, working directory, and recoverable environment are reused. Occupied historical ports block a relaunch; success requires the new process tree to own the expected ports. Commands without a saved, recoverable launch context explain why recovery is unavailable.
