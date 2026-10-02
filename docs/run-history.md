# Run History and Favorite Launch Points

Run History is the actionable command layer above Command Recovery. Port Timeline remains the forensic record of socket ownership; Run History answers which commands were executed and whether an exact captured launch can be run again.

## Data flow

```text
Shell Integration or Command Recovery
  -> LaunchContext
  -> CommandFingerprint
  -> CommandRun
  -> observed process and ports
  -> Run History / Favorite commands / Command palette
```

`CommandFingerprint` includes project identity, executable, literal arguments, working directory, execution type, and shell. It never includes a PID. Repeated executions therefore share one reusable launch context while retaining separate run records.

The SQLite database adds:

- `command_runs` for individual executions and their outcomes.
- `run_ports` for ports observed during a run.
- `favorite_commands` for explicit per-port pinned defaults.

These tables share `port-timeline.sqlite`; no second history store or command executor is introduced. Normal retention can remove old run records. Launch contexts and pinned favorite links remain separate so an explicitly saved launch point is not deleted with its old executions.

## Run Again preflight

Run Again uses the in-memory, secret-bearing capture owned by Command Recovery. Before spawning anything it checks:

1. The original working directory still exists.
2. The exact executable is still available.
3. The capture remains recoverable.
4. An equivalent command is not already active.
5. Previously observed ports are not occupied.

The command is passed as a structured executable and argument vector to the existing detached supervisor. Display strings are never parsed back into a command. When prior ports are known, success requires the new process to remain alive and those ports to appear under the new launch tree across multiple scans. A successful spawn by itself is not reported as a successful service start.

## Environment safety

Secret-bearing environment values stay in memory and are transferred only through the existing private supervisor pipe. Durable launch metadata remains redacted and is deliberately marked unrecoverable after an application restart unless a project recovery profile supplies safe execution context. Port Authority never substitutes another package manager or silently guesses missing shell state.

## Favorites

A favorite port lists commands that historically owned it. When the port is available, its most recent or explicitly pinned command can be run directly. When occupied, Favorites offers the current process actions and does not present an accidental duplicate launch. Pinning is always explicit; recency alone does not create a permanent default.
