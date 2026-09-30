# Conflict Autopilot

Conflict Autopilot turns a failed local development command into a verified, user-directed recovery. It does not scrape terminals, inspect shell history, or silently stop processes.

## Enable it

1. Open **Autopilot** in the desktop sidebar and enable **Optional shell integration**.
2. Copy the setup shown by the app into a bash or zsh session. The paths point to your actual executable and the bundled integration script.
3. Run your usual development command:

   ```sh
   npm run dev
   ```

The setup runs `pa_autopilot_on`, which intercepts conventional `dev` and `start` invocations of npm, pnpm, yarn, and bun in **that shell session only**. Other invocations bypass capture. Existing package-manager aliases/functions are preserved: activation refuses to replace them. `pa_autopilot_off` removes the installed wrappers, while preserving any functions you subsequently redefined.

For explicit per-command capture, source the script without running `pa_autopilot_on`, then use:

```sh
pa npm run dev
pa python3 -m http.server 8000
```

No `.bashrc`, `.zshrc`, startup service, or shell history file is modified. The helper preserves the command's nonzero exit status. Nothing is retried automatically. Disabling Autopilot clears pending captures; already running recovered services continue. The opt-in preference is stored locally and restored on the next desktop launch.

## What gets captured

The Rust helper launches an external executable and tees stdout/stderr to the terminal. At launch it also sends the argument vector, working directory, exported environment, and PID/start identity to Command Recovery, so successful commands can later be restarted. It reports a conflict only when the command fails and its output identifies exactly one valid occupied port. Recognized evidence includes `EADDRINUSE`, explicit `Port 5173 is already in use` failures, and address-already-in-use errors with an IPv4/IPv6 endpoint or `port:` field. A framework that successfully chooses another port does not trigger a conflict. Ambiguous or missing ports are not guessed.

This milestone recovers TCP development listeners. Explicit datagram errors are not captured, and a port also used by UDP is not eligible for owner termination.

The exact argument vector, resolved executable, working directory, and exported environment are captured before launch. The full environment crosses only a private same-user Unix-domain socket; it is held in native memory and never included in frontend responses, saved to disk, or deliberately logged. Commands and bounded diagnostic output are visible in the UI with credential-shaped arguments and captured private environment values redacted.

Capture is intentionally limited to **external, noninteractive development commands**. Shell aliases, functions, pipelines, redirections, shell options, umask, inherited file descriptors, and terminal state are not reproduced. Arguments are passed directly; the wrapper does not evaluate shell syntax. Use a real executable and arguments. Non-UTF8 arguments are rejected with guidance; environments/paths containing non-UTF8 data run normally but cannot be handed off for retry. Retried commands use captured stdout/stderr and no interactive stdin. A detached supervisor keeps output pipes functional after Port Authority closes. This is not a PTY or a terminal replacement.

## Recovery behavior

**Kill & Retry**:

1. Validate that the original executable and working directory still exist.
2. Freshly inspect the port owner, comparing PID, start time, executable, arguments, working directory, user, parent PID, and protection flags with the captured owner.
3. Classify risk again. Refuse protected, privileged, different-user, critical, or insufficiently identified owners. Require confirmation for infrastructure, unknown processes, cross-project owners, and owners not previously observed.
4. Recheck ownership immediately before the existing independent PID/start-time termination guard; send a graceful signal. For captured owners, Command Recovery stops the verified launch root and descendants before retrying.
5. Wait up to three seconds, scanning and checking IPv4/IPv6 bind availability. If a replacement owner appears, stop recovery. A stubborn original owner produces **Force Kill & Retry**, requiring a separate explicit confirmation and another complete ownership check.
6. Recheck availability, then launch the original command with an explicit argument vector, original directory, and a cleared/restored environment.
7. Capture the launcher PID and output. Verify that the desired listening socket belongs to that process or a currently validated descendant, not an unrelated process that happened to claim the port.

Success is reported only after verified ownership. A failed retry displays its actual exit status and diagnostic output. If the process remains alive but does not own the port after 15 seconds, the result is **unverified**, never success. A second launch is rejected while that attempt remains alive. The resulting listener can be inspected in the existing drawer.

**Use another port** never stops the owner. It supports explicit `--port` rewrites for direct Vite / `next dev` commands and simple `npm run <script>` scripts whose body is exactly `vite`, `vite dev`, or `next dev`. npm's `--` delimiter and an existing `--port`/`--port=` option are handled; Next's `-p` is also recognized. Compound scripts, arbitrary flags before the npm delimiter, workspace selectors, and unknown tools are not rewritten. The next available port within 100 ports is suggested and checked again before launch. The executed retry command is shown in the details.

**Restart Owner** is available when Command Recovery has a complete runtime launch context or an explicit project fallback recipe for the owner. It requires confirmation, safely stops the validated launch tree, waits for its processes and ports to exit, and relaunches that owner's captured command. It does **not** retry the blocked project. If a supervisor stays alive, recovery stops instead of creating a duplicate. Merely observing a process is not enough to reconstruct its environment.

**Inspect** opens the existing process drawer. **Ignore** discards the pending capture and leaves all processes alone. **Retry without killing** is available when the user has freed the port manually; it still requires a fresh availability check.

## Transparent classification

The reusable native classifier is in `src-tauri/src/autopilot/classifier.rs`. It uses executable identity, command argument basenames, user/protection metadata, parent PID, working directory, process age, and prior observation. It never identifies a service solely from a port number.

| Category | Typical handling |
| --- | --- |
| `DEV_SERVER` | Explicit one-click recovery only when metadata is complete, previously observed, and in the same project |
| `USER_PROCESS` | Review and confirmation |
| `INFRASTRUCTURE` | Extra acknowledgement; examples include PostgreSQL, MySQL, Redis, MongoDB, Docker, nginx |
| `SYSTEM_PROCESS` | Blocked, including critical services and other users' processes |
| `UNKNOWN` | High-risk review, or blocked when executable/user/start-time/command identity is missing |

Risk levels are internal policy, not numerical certainty scores. “Looks like a development server” is deliberately qualified. Project attribution uses the nearest package/repository marker, falling back to the observed working directory. “Possibly left over” requires a recognized development server over three hours old in a different project; age alone does not establish staleness or authorize termination.

Socket ownership checks and Unix signals cannot be made one atomic OS operation. Fresh metadata, repeated socket checks, start-time guards, conservative classification, and refusal to act on changed identities reduce that race; the app does not claim an absolute guarantee. A supervisor can restart a service, and a free port is not reserved while the command starts.

## Local transport and lifecycle

- Unix socket: `/tmp/port-authority-<uid>/autopilot.sock`.
- Parent directory is checked for same-user ownership and private `0700` permissions; the socket uses `0600`. Both ends verify peer UID using macOS/Linux OS credentials. Root capture is rejected.
- Existing live socket servers are not replaced; stale sockets are removed only after ownership/type checks and a refused connection.
- Requests have size and I/O time limits. No TCP listener, external network service, or bearer secret in shell startup files is used.
- Pending captures expire after 15 minutes. At most 32 recent conflicts and eight live recovered commands are retained. Active launch contexts remain in memory for verified owner restart until those launchers exit or the app quits.
- stdout/stderr use a bounded 32 KiB memory buffer. Command Recovery retains up to 200 redacted output artifacts in private local files and saves redacted launch metadata and project recipes in the existing timeline database. Full runtime environments are never persisted. Only the opt-in preference enters browser storage. Port Timeline separately records observed ownership transitions when enabled, including snapshots of the listener’s process command and project metadata. View Port History exposes repeated returns and persistent ancestry before another kill/retry.
- Operations are serialized; double clicks cannot start concurrent recoveries. Ignore and opt-out are refused during an active recovery, rather than discarding its context halfway through.
- The browser preview uses clearly labeled, separate simulations; it never invokes native process recovery.

macOS is exercised locally. Linux follows the same Unix integration and is included in CI. Windows keeps the existing port inspector, but shell capture is explicitly unavailable in this milestone.

## Verification

```sh
npm test
npm run test:shell
npm run test:e2e
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --debug --bundles app
```

The native integration tests create disposable server processes. They cover graceful recovery, context preservation, changed-owner refusal, forced escalation, alternate-port recovery without stopping the owner, managed-owner restart, genuine retry failure, unrelated listeners, duplicate-launch blocking, expiry, output parsing, classification, and private IPC. The ignored Rust test is a subprocess fixture explicitly invoked by those tests, not a skipped feature test. Shell tests run both bash and zsh when installed. Browser tests cover review/inspection, cross-project and infrastructure confirmations, force escalation, alternate recovery, and workspace banners.
