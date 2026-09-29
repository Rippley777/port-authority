# Port Timeline

Open **History**, or choose **View Timeline** from a port’s action menu or process drawer. Conflict Autopilot’s **View Port History** opens the same port history and explains observed recurring activity before another kill/retry.

The desktop recorder stores claims, releases, direct ownership changes, likely restarts, reclaims, and monitoring baselines. Each binding is identified by protocol, local address, and port; TCP/UDP, IPv4/IPv6, and multiple owners remain distinct. Search covers saved process/project metadata, commands, paths, repositories, PIDs, and event names. Events are paginated in observation order, with stable IDs. While reading older records, new activity waits behind **Show new activity**.

## Restart evidence

A different `(pid, started_at)` on the same binding can be a likely restart when executable, nonempty command, and project root (or working directory) match. Release/reclaim pairs must be within 60 seconds to be called a restart. Other returns within the configurable detection window are reclaims. Changing projects is an ownership change, not a restart. OS start times currently have sysinfo’s second resolution; unavailable process identities cannot substantiate restart correlation or destructive actions.

By default, three restarts/reclaims of the same service within ten minutes trigger **Port keeps returning**. The nearest ancestor whose PID **and start time** match in every old/new owner snapshot is shown as a possible source. This is evidence of common ancestry, not proof that the ancestor issued every restart. Docker proxies and other supervisors are shown as the observed host processes; the recorder does not infer container-internal ownership.

**Inspect Parent / Process Tree** refreshes the live tree. **Stop Process Tree…** opens that same review and requires an explicit acknowledgement. The backend checks the root identity, every reviewed member, ownership/protected-process rules, and whether the tree changed before sending graceful termination signals. It never automatically stops anything and never escalates to force. A partially completed stop can still report an error if a member exits or changes during the operation.

## Observation boundaries

Timeline timestamps mean “observed on this scan,” not the exact instant the OS changed. Processes that appear and disappear entirely between scans cannot be recovered. Pausing, scan failures, disabling history, app restarts, backwards clock movement, and scan gaps longer than 30 seconds create observation boundaries. Sleep/wake resumes from a fresh baseline; it does not invent intermediate events.

Startup compares the last durable owners to the new scan. Different owners are explicitly labelled **Owner changed while monitoring was offline**. A stable owner is observed again rather than implying uninterrupted ownership. Ownership sessions end at the monitoring session’s last observation, never across an unknown interval. “Available” intervals are scoped to a specific binding and only begin when a scan confirms no remaining owner of that binding.

There is deliberately no disk heartbeat on unchanged scans. Clean pause/exit saves the last successful scan time. After a crash, the last durable observation is a conservative boundary; the exact stop time is unavailable. This can overstate the unknown interval but cannot claim continuous observation that did not occur.

## Storage and performance

`port-timeline.sqlite` lives in Tauri’s per-user application-data directory. It uses SQLite WAL, batched transactional event writes, indexed port/time, identity, project/time, event/time and sequence queries, plus a single current-owner checkpoint. The checkpoint is replaced only on transitions or monitoring-session boundaries. An unchanged scan writes nothing to the history database.

Discovery remains in `ports/scanner.rs`; `timeline/reconciler.rs` compares observations; `correlation.rs` evaluates restart/ancestry evidence; `repository.rs` stores and queries records. Tauri performs database and process inspection work in the blocking worker pool. Project snapshots reuse Project Awareness’s resolver and caches. Ancestry is captured for changed processes and reused for unchanged bindings. A failed transaction does not advance reconciliation state, and a history failure leaves port inspection operational. If the database cannot be opened at startup, History reports the error; restoring storage and restarting the app reopens it.

Retention defaults to **30 days**, with 24 hours, 7 days, 90 days, and forever options in **Settings → Port history**. Queries hide expired events immediately; maintenance deletes expired rows hourly during recording and when retention changes. SQLite reuses freed pages; file size need not fall immediately. **Clear History** deletes events, session metadata, and the current-owner checkpoint, checkpoints WAL, and compacts the database. Disabling recording preserves existing history. A legacy “keep history” opt-out is migrated before the first desktop scan. The old browser started/stopped log is not imported as forensic evidence: it lacks binding and identity guarantees.

Commands and paths are sensitive local data: nothing is uploaded and no telemetry is introduced. Watched ports use the same detailed recorder as other bindings. Browser preview uses sample history and cannot control real processes.

## Scope and verification

The initial feature includes the complete claim → release → restart sequence, repeated-return explanation, saved ancestry and live tree review, global/per-port search, ownership sessions, retention, and monitoring gaps. Dedicated conflict/action event types, notifications, a graphical scrubber, and per-watched-port retention are deferred.

Run:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run test:e2e
npm run build
```

Backend tests exercise identity reuse, missing identity, distinct/shared bindings, direct and separated restarts, project changes, recurring ancestors, sleep/clock gaps, persistence, rollback, retention, disabled/idle write behavior, and process-tree guards. Browser tests cover filters, historical details, direct navigation, retention/clearing, confirmation, pagination, stable live refresh, storage errors, and narrow layouts.
