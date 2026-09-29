# Port Authority

**Know what’s listening.** A local-first desktop utility for inspecting listening TCP ports, bound UDP sockets, and the processes that own them. Built with Rust, Tauri 2, React, and strict TypeScript.

![Port Authority’s ports view](docs/ports.png)

## Run

Requires Node.js 20+ (22 recommended), npm, and Rust. `rust-toolchain.toml` selects Rust 1.95.0. Install the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/) first: Xcode command line tools on macOS, WebKitGTK 4.1 and build tools on Linux, or MSVC build tools and WebView2 on Windows.

```sh
npm ci
npm run desktop
```

The desktop app inspects **real local sockets**. To explore the UI without native dependencies:

```sh
npm run dev
# http://127.0.0.1:1420
```

The browser version is explicitly labeled **Preview workspace**. It uses sample services; killing a sample process only changes the preview. Browser actions still open the selected localhost URL when requested. Preview history is stored separately from native history. Native copy actions use the [Tauri clipboard plugin](https://v2.tauri.app/plugin/clipboard/) with write-only permission.

For a desktop package:

```sh
npm run desktop:build
# macOS app: src-tauri/target/release/bundle/macos/Port Authority.app
```

A locally verified, unsigned debug app is also available after:

```sh
npm run tauri build -- --debug --bundles app
```

Quit any separately started Vite server on port 1420 before running `npm run desktop`, which starts its own frontend server.

## Conflict Autopilot

Opt-in bash/zsh integration detects failed commands with occupied-port errors and offers guarded **Kill & Retry**, **Use another port**, **Inspect**, and captured **Restart Owner** actions. Open **Autopilot** in the desktop sidebar to enable it and copy the setup for your shell. No startup files are edited.

See [Conflict Autopilot](docs/conflict-autopilot.md) for setup, safety policy, supported command adapters, and lifecycle limits. The browser preview includes labeled development-server, infrastructure, and force-escalation simulations.

## What works

- Native IPv4/IPv6 socket discovery, process/PID association, and process details.
- Two-second live refresh; pause, manual refresh, and configurable intervals.
- Stable row identities, numeric sorting, instant search, exact `:5173` searches, and protocol/interface/owner filters.
- Individual port and grouped process views; process details drawer and keyboard-accessible context menus.
- Graceful termination on Unix; force termination with mandatory confirmation. Privileged, different-user, critical, and application-owned processes are protected.
- Browser launch for common HTTP development ports, explicit confirmation for other ports, and copy actions.
- Persistent favorites, including inactive watched ports; port availability lookup.
- Overview, bounded local history, settings, and command palette.
- Locally bundled fonts. No analytics, telemetry, cloud service, account, or remote requests on startup.

| Shortcut | Action |
| --- | --- |
| Cmd/Ctrl K | Command palette |
| Cmd/Ctrl F | Filter ports |
| Cmd/Ctrl R | Refresh |
| Cmd/Ctrl , | Settings |
| Escape | Close drawer or dialog |

## Architecture

```text
src/
  components/        Table, drawer, dialogs, navigation, and secondary views
  hooks/             Monitoring, application state, validated persistence
  lib/               Typed bridge, pure filtering/reconciliation, preview fixtures
src-tauri/src/
  ports/             Core scanner, serializable socket/process models
  process/           Cached metadata policy and guarded process controller
  platform/          Native discovery adapter, Unix and Windows termination
  commands/          Thin, asynchronous Tauri command adapters
  autopilot/         Conflict detection, classification, capture, recovery, and private IPC
shell/               Optional bash/zsh integration
```

The discovery and control modules do not depend on Tauri. The desktop commands move blocking work to worker threads. Socket discovery uses [netstat2](https://docs.rs/netstat2/0.11.2/netstat2/); process inspection uses [sysinfo](https://docs.rs/sysinfo/0.33.1/sysinfo/). No discovery shell commands are spawned. macOS uses libproc, Linux uses netlink/procfs, and Windows uses IP Helper APIs through the shared socket adapter. Platform-specific process control stays in `platform/`.

A retained `sysinfo::System` caches slow-changing command/path/user metadata and provides CPU sampling across scans. The scanner refreshes relevant and cached process IDs, removes dead processes, and caps its cache at 4,096 entries. Scans never overlap in the UI. Full bounded snapshots cross IPC; the frontend reconciles unchanged row objects rather than resetting the table. Delta-only IPC and virtualization are future optimizations if profiling requires them.

Settings, favorites, and history use the app webview’s local storage with validation and safe defaults. History retains 500 events by default, configurable to 100 or 1,000. Disabling history clears stored events. There is no persistent process-command log. CPU is initially zero until a second scan; process uptime is the process age, not socket age.

## Safety and platform behavior

Before termination, the backend independently re-reads the process, validates its start time against the selected PID, checks ownership, and rejects critical services and Port Authority itself. The frontend always confirms force kill; graceful confirmation is configurable. There is no automatic privilege escalation. Termination may be asynchronous; the next scan reflects whether the service actually exited. A supervisor may relaunch it.

| Platform | Implementation | Verification in this workspace |
| --- | --- | --- |
| macOS | Native sockets and metadata; SIGTERM / SIGKILL | Compiled, tested against real TCP/UDP sockets and disposable child processes; app bundled and launched |
| Linux | Native socket adapter and procfs; SIGTERM / SIGKILL | Implemented; CI workflow provided, not executed here |
| Windows | Native socket adapter; handle-based force termination with creation-time validation | Implemented; CI workflow provided, not executed here |

Windows has no universal graceful signal for arbitrary processes. Normal Kill returns a useful explanation; **Force Kill** uses `OpenProcess` / `TerminateProcess` and never silently substitutes for graceful termination. Missing process information is omitted, with an explanation in the drawer. Unknown-owner sockets remain visible and cannot be terminated.

“Available” means no matching socket was visible in the most recent successful scan. It is not a reservation or a guarantee that an immediate bind will succeed; OS permissions can limit visibility. Failed scans never produce an availability claim. Paused lookups explicitly refer to the last scan.

The general process drawer keeps restart disabled for observed processes: executable, arguments, and working directory alone cannot reproduce the environment or supervisor. Conflict Autopilot can restart owners it actually launched from a complete captured context. Startup registration is visibly unavailable; System theme currently uses the dark appearance. The initial package is unsigned/unnotarized. There is no general-purpose CLI; the executable provides only the dedicated shell-capture entry point used by `pa`.

## Verify

```sh
npm run build
npm run lint
npm test
npm run test:shell
npx playwright install chromium
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Tests cover query validation, search, filtering, recognition, sorting, stable reconciliation, persistence validation, primary browser flows, clipboard and browser actions, scan failure handling, actual socket discovery, PID identity guards, critical-process protection, and graceful/force termination of test-owned children. The GitHub Actions workflow defines frontend checks and a three-platform Rust matrix.

MIT licensed.
