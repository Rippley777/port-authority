# Building and releasing Port Authority

Port Authority uses Tauri 2, Rust, React/TypeScript, Vite, and npm. The release pipeline extends the existing Tauri build; local development does not depend on GitHub Actions.

## Prerequisites and versions

- Node.js 20+ (22 recommended) and `npm ci` using `package-lock.json`.
- Rust/rustup; `rust-toolchain.toml` pins the toolchain and components. Cargo builds use `--locked` and `src-tauri/Cargo.lock`.
- Install the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/): Xcode/command line tools on macOS, MSVC C++ build tools and WebView2 on Windows, WebKitGTK 4.1 and native build tools on Linux.
- Linux release packaging requires an x64 host. CI uses Ubuntu 22.04 to avoid unnecessarily raising the glibc baseline. AppImages still depend on host graphics/system libraries; test on supported distributions before publishing.

The release filename version comes from `src-tauri/tauri.conf.json`. The script requires `package.json` and `[package].version` in `src-tauri/Cargo.toml` to match. When bumping a version, update all three and their lockfile package entries. Version tags must be `v<version>` (for example, `v0.1.0`). No automatic version bumps occur.

## Development

```sh
npm ci
npm run desktop          # Native app + Vite development server
npm run dev              # Browser preview with sample sockets
npm run build            # Type check + production frontend in dist/
```

`npm run build` produces the frontend only. For an optional debug desktop bundle:

```sh
npm run archive:builds
npm run tauri build -- --debug --bundles app
```

Raw Tauri commands bypass the release wrapper. Always archive first if using them to replace an existing package. Prefer the commands below for production bundles.

## Production commands

```sh
npm run build:release    # Current host: both macOS architectures, or Windows/Linux x64
npm run desktop:build    # Alias for the safe release pipeline
npm run build:mac        # macOS arm64 + x64, requires a Mac
npm run build:mac -- --arch arm64
npm run build:mac -- --arch x64
npm run build:windows    # Windows x64 NSIS installer, requires Windows
npm run build:linux      # Linux x64 AppImage + Debian package, requires Linux
npm run archive:builds   # Archive existing distributables without building/cleaning
npm run test:release    # Archive, failure, lock, repeat-release and CI assembly tests
```

Install missing Rust targets before building:

```sh
# macOS: both targets can be compiled on Apple Silicon or Intel with the macOS SDK.
rustup target add aarch64-apple-darwin x86_64-apple-darwin
# On the respective native hosts:
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-unknown-linux-gnu
```

An Intel Mac is not required for an Intel package. Separate architecture builds are the default so downloads remain explicit. Tauri also supports `universal-apple-darwin`, but combining both slices increases each download and is unnecessary here. Legacy universal bundles are preserved by the archive scanner. See [Tauri's cross-platform CI example](https://v2.tauri.app/distribute/pipelines/github/).

Windows and Linux packages use native runners, avoiding extra cross-compilation toolchains and installer hacks. Asking for another operating system on a local host fails before cleanup. The summary distinguishes built targets from platforms skipped because they require another host; a compiler/bundler failure exits nonzero.

Use `--verbose` for underlying Tauri/bundler diagnostics. Builds use noninteractive mode (`CI=true` for the Tauri subprocess), including skipping Finder layout AppleScript for DMGs. macOS packaging requires disk-image mounting access; an OS sandbox that blocks `hdiutil` cannot produce DMGs. This does not affect ordinary unsandboxed terminal builds.

The Node scripts resolve the repository from their own location, use argument arrays without shell interpolation, and support paths containing spaces. From another directory:

```sh
npm --prefix "/absolute/path/port-authority" run build:release
# Or invoke the script directly:
node "/absolute/path/port-authority/scripts/release.mjs" release
```

## Outputs

A successful local invocation replaces `builds/current/` as one release, with only that invocation's requested targets. An individual architecture build therefore archives the previous complete output and leaves just that architecture in `current`; it does not merge stale packages. CI assembles all platforms from one commit.

```text
builds/current/
  BUILD_INFO.json
  SHA256SUMS.txt
  macos/
    Port-Authority-0.1.0-macos-arm64.app
    Port-Authority-0.1.0-macos-arm64.app.tar.gz
    Port-Authority-0.1.0-macos-arm64.dmg
    Port-Authority-0.1.0-macos-x64.app
    Port-Authority-0.1.0-macos-x64.app.tar.gz
    Port-Authority-0.1.0-macos-x64.dmg
  windows/
    Port-Authority-0.1.0-windows-x64.exe
  linux/
    Port-Authority-0.1.0-linux-x64.AppImage
    Port-Authority-0.1.0-linux-x64.deb
```

Raw `.app` bundles are available locally. CI transports `.app.tar.gz` instead: tar preserves executable permissions and bundle symlinks that artifact ZIP transport would lose. Extract with `tar -xzf <file>.app.tar.gz`. The inner bundle name also includes the version and architecture; its displayed product name remains Port Authority.

`BUILD_INFO.json` includes version, UTC timestamp, full Git commit, branch (null for detached HEAD or unavailable Git), Git ref, dirty working-tree status, host, requested targets, archive location, signing mode, and artifact tree hashes. Git is optional for local builds/archives. A dirty working tree is explicitly recorded; its contents are not reconstructible solely from the commit. `SHA256SUMS.txt` contains standard SHA-256 file checksums for installers/tarballs (not directories). Check with `shasum -a 256 -c SHA256SUMS.txt` on macOS or `sha256sum -c SHA256SUMS.txt` on Linux from `builds/current`.

## Archive safety and failure handling

Before any desktop compilation or cleanup, the pipeline:

1. Validates the host, tools, installed Rust targets, version consistency and tag.
2. Discovers `builds/current`, leftover staging/previous output, existing packages in `artifacts/`, and Cargo bundle directories. Cargo metadata locates configured/custom target directories; the default `src-tauri/target` is also scanned.
3. Reserves `builds/archive/YYYY-MM-DD_HH-mm-ss/` exclusively. Names use **UTC**. Same-second collisions receive `-001`, `-002`, etc.; previous archives are never overwritten or deleted.
4. Copies each distributable, then verifies its contents, relative tree paths, executable modes and symlinks against the unchanged source. `.app` trees are copied without following links outside the bundle. No Git checkout is required.
5. Only after all copies pass, cleans frontend output, old staging output and the selected targets' bundle directories. Rust compilation caches remain reusable.
6. Builds into Tauri's standard target directories, checks expected formats, copies normalized output into `builds/.staging`, and generates checksums and metadata.
7. Replaces `current` after every requested target succeeds. If compilation fails, old `current` remains available; partial output and failure metadata stay in `.staging` and are archived by the next invocation.

Archive metadata marks `status: complete` only after verification. Failed copies leave a partial archive marked `archiving` and preserve source builds; investigate and retry, without deleting completed archives. `PREVIOUS_BUILD_INFO.json` retains the previous current's exact metadata when available. Historical framework packages live below `<platform>/legacy/<original path>` to keep provenance and avoid name collisions. The archive's version is the checkout version at archive time; older package versions remain in original filenames and previous metadata.

Only distributables are archived: `.app`, `.dmg`, `.exe`, `.msi`, `.AppImage`, `.deb`, `.rpm`, `.zip`, `.tar.gz`, and accompanying `.sig` files. Packaged debug `.app` bundles are preserved, but `target/debug` binaries/dependencies, `node_modules`, compilation caches and generated frontend files are excluded. Incomplete `rw.*` DMG scratch images are excluded. Existing source packages are copied rather than moved, so repeated archives can include them again; only selected build output is cleaned.

`builds/` and the legacy `artifacts/` are ignored by Git. Archives remain local until you deliberately back them up. Nothing prunes archives automatically. A directory lock at `builds/.release.lock` prevents concurrent archive/release operations. After an interrupted process, confirm no release is running before manually removing that lock. If power loss leaves `builds/.previous`, restore it to `current` if needed; the next invocation archives it before cleanup.

## GitHub Actions

`.github/workflows/check.yml` retains the existing frontend/native checks and adds the release-tool tests. `.github/workflows/release.yml` runs manually through **Actions → Release → Run workflow** or on a pushed `v*` tag:

- `macos-latest`: installs both Rust targets, builds arm64 and x64 `.app`/DMG packages.
- `windows-latest`: builds an x64 NSIS `.exe` on MSVC.
- `ubuntu-22.04`: installs native dependencies and builds x64 AppImage/`.deb`. `APPIMAGE_EXTRACT_AND_RUN=1` avoids requiring FUSE mounting during packaging.
- Each runner uses `npm ci`, unit/release tests, native Rust tests and the same `build:release` command used locally. Each uploaded platform artifact contains its metadata and checksums.
- The collection job selects the newest successful upload per platform on reruns, verifies file checksums, version, architectures and matching Git commits, and uploads one complete release artifact. CI's fresh checkouts normally have nothing to archive; archives are not persisted in Rust caches.
- Manual runs upload downloadable Actions artifacts without creating a GitHub Release. Version-tag runs also create a **draft** GitHub Release containing installers, app tarballs, consolidated metadata and checksums. Review it before publishing. Actions artifacts expire after 30 days; draft release assets persist.

Tagging/pushing is not performed by the local build command. Update versions and commit the reviewed changes before creating a tag. The existing Check workflow remains independent.

## Signing and notarization

No signing credentials are needed to make local/test distributables. macOS uses an **ad-hoc** signature (`signingIdentity: "-"`); this is not a trusted Developer ID signature or notarization. Windows installers are unsigned. Downloaded macOS bundles can be blocked by Gatekeeper, and Windows can display SmartScreen warnings.

For a trusted macOS release, use `npm run build:mac -- --signed` and provide a Developer ID identity via `APPLE_SIGNING_IDENTITY`. Tauri can import a CI certificate using `APPLE_CERTIFICATE` (base64 `.p12`) and `APPLE_CERTIFICATE_PASSWORD`. Notarization accepts `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), and `APPLE_TEAM_ID`, or `APPLE_API_KEY`, `APPLE_API_ISSUER`, and `APPLE_API_KEY_PATH`. Use CI secrets/environment variables, never repository files containing credentials. See [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).

`--signed` removes the local ad-hoc override and lets Tauri use configured credentials; it does not provision certificates or promise that a missing configuration is signed. To enable it in CI, add the secrets to the packaging job environment and pass `--signed` to that job's release command. The shipped workflow deliberately requires no secrets.

Windows signing uses an installed certificate and Tauri's `bundle.windows` signing configuration or a `signCommand` supplied by a platform config (`src-tauri/tauri.windows.conf.json`). Configure certificate thumbprint/digest/timestamp or a signing service using protected environment/CI secrets when you have a certificate. See [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/). Linux packages do not require a signing identity; checksums are supplied.

## Verification

```sh
npm run format:check
npm run lint
npm test
npm run test:shell
npm run test:release
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run build:release
```

Run two releases to check that the second receives a new archive and `current` contains only the newest requested targets. Inspect `BUILD_INFO.json`, verify checksums, inspect macOS binaries with `file`/`lipo -archs`, and launch the normalized `.app`. Intel runtime testing requires Intel hardware or Rosetta on Apple Silicon; producing the package does not install Rosetta. Test real socket discovery and normal product flows on each native platform before publicly publishing.
