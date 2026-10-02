# macOS process inspection and App Data privacy

## Confirmed cause (October 1, 2026)

A one-second `sample` of the running debug bundle (PID 6124) showed its project worker blocked throughout all 669 samples on this stack:

```text
ProjectEngine::resolve_entries
  resolver::candidate
    resolver::find_root
      manifests::markers
        Path::read_dir / std::fs::read_dir
          __opendir2
            open$NOCANCEL / __open_nocancel
```

The concrete offending operation was the unconditional directory enumeration in `projects/manifests.rs`, used to find arbitrary `.sln` and `.csproj` names. Project discovery reached it from process cwd, executable, argument and parent paths. These could point into another application's protected directory. A failed lookup was then retried after the project cache's 30-second expiry. The sample establishes the blocking operation; it does not expose the target pathname. We have not invented a pathname or blamed libproc socket discovery.

Existing unified logs independently confirm `kTCCServiceSystemPolicyAppData` requests and prompts for Port Authority. At 20:35:25 Central, TCC reported a failed code-requirement match and prompted the debug app. At 20:34:55 it reported the equivalent failure for the installed app.

Both copies use `dev.portauthority.desktop`. `codesign -dr -` showed different hash-only designated requirements for the installed and debug binaries. The installed app has an ad-hoc/linker signature, no TeamIdentifier and an unbound Info.plist. Running differently built copies under the same bundle identity caused grants to alternate between incompatible code requirements. A stable bundle ID alone does not make an ad-hoc signature stable across rebuilds.

No identifier changes, entitlements, Full Disk Access requests, private TCC APIs or automatic System Settings navigation were added.

## Access policy

- Core macOS discovery uses netstat2's descriptor/socket APIs and `PROC_PIDTBSDINFO` for process identity, name, parent ID and owner. `PROC_PIDTASKINFO` supplies optional memory and CPU counters. It does not request cwd, argv or environment.
- The project worker separately resolves optional kernel metadata once per PID + start time. Cwd uses `PROC_PIDVNODEPATHINFO`, which returns a path string without opening it. Command/executable strings use sysinfo; missing values are marked Unknown because sysinfo does not expose errno.
- Cwd denial, protected paths and unsupported metadata are remembered. There is no denial timeout or exponential retry. Exited/replaced identities are pruned; the metadata cache is capped at 4,096 identities and stops admitting new entries at capacity rather than evicting denials and retrying them.
- Filesystem inference requires an available cwd. No executable, argv or parent-directory fallback is attempted. Protected directory spellings (including Library, application bundles, standard personal-data directories and volumes) are rejected before filesystem lookup. Untrusted symlinks are not followed. Standard macOS `/tmp` and `/var` aliases are normalized lexically.
- Project detection checks known manifest names without enumerating directories. .NET detection checks the directory's name with `.sln`/`.csproj` suffixes. Arbitrarily named solution files alone no longer trigger automatic attribution.
- Project results, including negatives, remain cached until identity/evidence changes or **Refresh metadata** is explicitly selected. Repeated resource permission failures are remembered across PIDs; one `EACCES`/`EPERM` does not establish application-wide TCC denial. No reliable public preflight for this particular consent was identified, and none is simulated.
- Core scan and timeline recording consume cached project data without synchronously waiting for filesystem/Git resolution. Late results are merged by socket identity and process start time.
- macOS Command Recovery prefers opt-in shell captures and explicit recipes. It does not read arbitrary processes' environments during monitoring. Uncaptured launch contexts remain unavailable. Explicit control operations still revalidate identity and available metadata before acting; cached enrichment never authorizes signalling a reused PID.
- Missing metadata is explained quietly in the process drawer. Ports, protocols, addresses, owners and status remain usable.

`sysinfo` itself may query `KERN_PROCARGS2` when constructing a Process even with minimal refresh flags. The macOS core scanner therefore uses the BSD structure directly. Control/ancestry identity helpers still use sysinfo without requesting cwd, executable or environment; they never traverse the resulting paths.

## Development diagnostics

Set `PORT_AUTHORITY_TRACE_INSPECTION=1` for a debug build, for example:

```sh
PORT_AUTHORITY_TRACE_INSPECTION=1 npm run desktop
```

Trace records contain PID, operation and result only. They never print file contents, argv, paths or environment values. Instrumented boundaries include cwd resolution, optional metadata, project detection, manifest opening, Git metadata, and protected/denied filesystem paths. The tracing code is inactive in release builds.

For a live recurrence, sample the affected PID while the dialog is visible and compare the stack with the trace and the narrowly scoped TCC log window. Do not use unrelated private file contents as diagnostics.

## Verification and remaining manual checks

Automated checks passed: 64 native tests (four subprocess fixtures intentionally ignored), 15 frontend unit tests, four shell integration tests, and 31 Playwright flows. Rust formatting/Clippy, repository Prettier checks, TypeScript checks, the production web build and a debug macOS app bundle also passed. Coverage includes real socket discovery, cached permission/unsupported results, PID reuse, protected path rejection, symlink refusal, missing-cwd behavior, async project resolution, process control and captured recovery.

Live post-fix acceptance is **pending**: automatic approval review rejected quitting the old running debug bundle, interpreting the requested leave-running test step as a prohibition on interruption. Approval to close the two old copies and launch the replacement was requested. No privacy authorization was reset, granted or changed. The existing running copies still contain the pre-fix code.

The original debug and installed app were investigated live. Neither is Developer ID signed. A Developer ID signed/notarized build cannot be represented by an ad-hoc build; release-signing verification requires the distribution identity.

Manual acceptance must exercise the rebuilt app, not an already-running old copy:

1. Quit old installed/debug copies so they cannot generate their own prompts.
2. Launch the rebuilt development app. Monitor for several minutes while visiting Ports, Processes, Projects, Favorites, History and Timeline; verify a disposable TCP/UDP listener remains visible.
3. Repeat with the rebuilt `.app`, then the actual signed distribution build when available.
4. Test with App Data access denied and allowed. The monitoring loop should not generate any App Data request in either state. This fix adds no enhanced-inspection permission button because core/project monitoring has no need to request that access.
5. If a user explicitly opens a protected location through a reveal/editor action and declines an OS prompt, continue monitoring and navigating. That action must not become a background retry.
6. Record the app path, signing requirement, permission state, observation duration and matching TCC request count. Do not claim the Allow/Don't Allow matrix passed solely from automated tests or absence of a prompt in one already-authorized run.
