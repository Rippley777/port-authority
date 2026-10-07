import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  lstatSync,
  readlinkSync,
  cpSync,
  rmSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
);
const platforms = { darwin: "macos", win32: "windows", linux: "linux" };
const targets = {
  macos: { arm64: "aarch64-apple-darwin", x64: "x86_64-apple-darwin" },
  windows: { x64: "x86_64-pc-windows-msvc" },
  linux: { x64: "x86_64-unknown-linux-gnu" },
};
const formats = {
  macos: [".app", ".dmg"],
  windows: [".exe"],
  linux: [".AppImage", ".deb"],
};
const artifactPattern =
  /\.(app|dmg|exe|msi|AppImage|deb|rpm|zip|tar\.gz|app\.tar\.gz)(\.sig)?$/;
const slash = (path) => path.replaceAll("\\", "/");

function command(root, executable, args, { capture = false, env = {} } = {}) {
  const result = spawnSync(executable, args, {
    cwd: root,
    shell: false,
    encoding: "utf8",
    stdio: capture ? "pipe" : "inherit",
    env: { ...process.env, ...env },
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${executable} ${args.join(" ")} failed: ${result.error?.message ?? result.stderr?.trim() ?? `exit ${result.status}`}`,
    );
  }
  return result.stdout?.trim();
}

function git(root, args) {
  try {
    return command(root, "git", args, { capture: true });
  } catch {
    return null;
  }
}

export function readVersion(root, { verifyTag = false } = {}) {
  const config = JSON.parse(
    readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const cargo = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
  const cargoVersion = cargo
    .split("[package]")[1]
    ?.split(/\n\[/)[0]
    ?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const version = config.version;
  if (
    typeof version !== "string" ||
    !/^\d+\.\d+\.\d+(?:-[\w.-]+)?(?:\+[\w.-]+)?$/.test(version)
  ) {
    throw new Error(
      "Set a literal semver version in src-tauri/tauri.conf.json.",
    );
  }
  if (pkg.version !== version || cargoVersion !== version) {
    throw new Error(
      `Version mismatch: Tauri=${version}, package.json=${pkg.version}, Cargo.toml=${cargoVersion}. Update all three before releasing.`,
    );
  }
  const expectedTag =
    process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : null;
  if (verifyTag && expectedTag && expectedTag !== `v${version}`)
    throw new Error(`Tag ${expectedTag} must match v${version}.`);
  return version;
}

function metadata(root, version, now) {
  return {
    version,
    timestamp: now.toISOString(),
    gitCommit: git(root, ["rev-parse", "HEAD"]),
    gitBranch: git(root, ["symbolic-ref", "--short", "-q", "HEAD"]),
    gitRef: process.env.GITHUB_REF_NAME ?? null,
    gitDirty: (git(root, ["status", "--porcelain"]) ?? "").length > 0,
    host: { platform: process.platform, architecture: process.arch },
  };
}

// Hash a whole .app tree, including relative names, executable modes and symlinks.
// Never follow a symlink into arbitrary developer data.
export function fingerprint(path) {
  const hash = createHash("sha256");
  function visit(entry, name) {
    const stat = lstatSync(entry);
    hash.update(
      JSON.stringify([
        slash(name),
        stat.mode & 0o777,
        stat.isDirectory()
          ? "directory"
          : stat.isSymbolicLink()
            ? "link"
            : "file",
      ]),
    );
    if (stat.isSymbolicLink()) hash.update(readlinkSync(entry));
    else if (stat.isDirectory())
      for (const child of readdirSync(entry).sort())
        visit(join(entry, child), join(name, child));
    else if (stat.isFile()) hash.update(readFileSync(entry));
    else throw new Error(`Unsupported artifact entry: ${entry}`);
  }
  visit(path, "");
  return hash.digest("hex");
}

function discover(directory) {
  if (!existsSync(directory)) return [];
  if (lstatSync(directory).isSymbolicLink())
    throw new Error(`Artifact directory must not be a symlink: ${directory}`);
  const found = [];
  for (const name of readdirSync(directory).sort()) {
    if (name.startsWith("rw.") || name === ".DS_Store") continue; // Incomplete create-dmg scratch images.
    const path = join(directory, name);
    const stat = lstatSync(path);
    if (artifactPattern.test(name)) {
      if (stat.isSymbolicLink())
        throw new Error(
          `Distributable must be a real file/bundle, not a symlink: ${path}`,
        );
      found.push(path);
    } else if (stat.isDirectory()) found.push(...discover(path));
  }
  return found;
}

export function bundleRoots(targetDirectory) {
  if (!existsSync(targetDirectory)) return [];
  const directories = [
    targetDirectory,
    ...readdirSync(targetDirectory, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => join(targetDirectory, entry.name)),
  ];
  return directories
    .flatMap((directory) =>
      ["release", "debug"].map((profile) => join(directory, profile, "bundle")),
    )
    .filter(existsSync);
}

function inferPlatform(path) {
  if (
    /\.(app|dmg|app\.tar\.gz)$/.test(path) ||
    /macos|darwin|arm64/i.test(path)
  )
    return "macos";
  if (/\.(exe|msi)$/.test(path) || /windows/i.test(path)) return "windows";
  if (/\.(AppImage|deb|rpm)$/.test(path) || /linux/i.test(path)) return "linux";
  return "other";
}

// Copies are verified before any caller is allowed to clean build output.
// An incomplete archive is retained on failure; existing artifacts remain intact.
export function archiveBuilds(
  root,
  {
    version = readVersion(root),
    now = new Date(),
    targetDirectory = join(root, "src-tauri/target"),
  } = {},
) {
  const current = join(root, "builds/current");
  const records = discover(current).map((source) => ({
    source,
    destination: relative(current, source),
  }));
  const legacyDirectories = [
    join(root, "artifacts"),
    join(root, "builds/.staging"),
    join(root, "builds/.previous"),
    ...new Set([
      ...bundleRoots(targetDirectory),
      ...bundleRoots(join(root, "src-tauri/target")),
    ]),
  ];
  for (const directory of legacyDirectories) {
    for (const source of discover(directory)) {
      const origin = slash(relative(root, source));
      // External Cargo target directories are kept under a safe, unambiguous prefix.
      const destination = join(
        inferPlatform(source),
        "legacy",
        origin.startsWith("../")
          ? join("cargo-target", relative(targetDirectory, source))
          : origin,
      );
      records.push({ source, destination });
    }
  }
  if (!records.length) return null;
  const archiveRoot = join(root, "builds/archive");
  mkdirSync(archiveRoot, { recursive: true });
  const stamp = now
    .toISOString()
    .slice(0, 19)
    .replace("T", "_")
    .replaceAll(":", "-");
  let archive;
  for (let suffix = 0; ; suffix++) {
    archive = join(
      archiveRoot,
      `${stamp}${suffix ? `-${String(suffix).padStart(3, "0")}` : ""}`,
    );
    try {
      mkdirSync(archive);
      break;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }
  }
  const info = {
    ...metadata(root, version, now),
    status: "archiving",
    artifacts: [],
  };
  if (existsSync(join(current, "BUILD_INFO.json"))) {
    cpSync(
      join(current, "BUILD_INFO.json"),
      join(archive, "PREVIOUS_BUILD_INFO.json"),
      { errorOnExist: true, force: false },
    );
  }
  const infoPath = join(archive, "BUILD_INFO.json");
  writeFileSync(infoPath, `${JSON.stringify(info, null, 2)}\n`, { flag: "wx" });
  try {
    for (const record of records) {
      const destination = join(archive, record.destination);
      mkdirSync(dirname(destination), { recursive: true });
      const before = fingerprint(record.source);
      cpSync(record.source, destination, {
        recursive: true,
        errorOnExist: true,
        force: false,
        verbatimSymlinks: true,
      });
      if (
        fingerprint(destination) !== before ||
        fingerprint(record.source) !== before
      )
        throw new Error(`Archive verification failed: ${record.source}`);
      info.artifacts.push({
        source: slash(relative(root, record.source)),
        path: slash(record.destination),
        treeSha256: before,
      });
    }
    info.status = "complete";
    writeFileSync(infoPath, `${JSON.stringify(info, null, 2)}\n`);
    console.log(`Archived ${records.length} distributables: ${archive}`);
    return archive;
  } catch (error) {
    throw new Error(
      `Archival failed; no build output was cleaned. Partial archive retained at ${archive}. ${error.message}`,
    );
  }
}

export function selectTargets(
  platform = platforms[process.platform],
  arch = platform === "macos" ? "all" : "x64",
  host = platforms[process.platform],
) {
  if (!targets[platform])
    throw new Error(
      `Unsupported platform ${platform}. Choose macos, windows or linux.`,
    );
  if (platform !== host)
    throw new Error(
      `${platform} packaging requires a native ${platform} host/CI runner; current host is ${host}.`,
    );
  const architectures =
    arch === "all" ? Object.keys(targets[platform]) : [arch];
  return architectures.map((architecture) => {
    const triple = targets[platform][architecture];
    if (!triple)
      throw new Error(
        `${platform} ${architecture} is not configured. Supported: ${Object.keys(targets[platform]).join(", ")}.`,
      );
    return { platform, architecture, triple };
  });
}

function validateEnvironment(root, plan) {
  if (Number(process.versions.node.split(".")[0]) < 20)
    throw new Error("Node.js 20+ is required.");
  if (!existsSync(join(root, "node_modules/@tauri-apps/cli/tauri.js")))
    throw new Error("Tauri CLI missing. Run npm ci first.");
  command(root, "cargo", ["--version"], { capture: true });
  const installed = command(root, "rustup", ["target", "list", "--installed"], {
    capture: true,
  }).split(/\s+/);
  for (const { triple } of plan)
    if (!installed.includes(triple))
      throw new Error(`Rust target missing. Run: rustup target add ${triple}`);
  if (plan[0].platform === "macos") {
    command(root, "xcrun", ["--find", "clang"], { capture: true });
    command(root, "tar", ["--version"], { capture: true });
  }
  if (plan[0].platform === "linux") {
    for (const library of [
      "webkit2gtk-4.1",
      "gtk+-3.0",
      "ayatana-appindicator3-0.1",
    ])
      command(root, "pkg-config", ["--exists", library], { capture: true });
    command(root, "patchelf", ["--version"], { capture: true });
    if (process.arch !== "x64")
      throw new Error(
        "Linux x64 AppImage packaging requires an x64 Linux host/runner.",
      );
  }
  const cargo = JSON.parse(
    command(
      root,
      "cargo",
      [
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--manifest-path",
        join(root, "src-tauri/Cargo.toml"),
      ],
      { capture: true },
    ),
  );
  return cargo.target_directory;
}

function buildTarget(root, target, signed, verbose, targetDirectory) {
  const args = [
    join(root, "node_modules/@tauri-apps/cli/tauri.js"),
    "build",
    "--ci",
    "--target",
    target.triple,
    "--bundles",
    target.platform === "macos"
      ? "app,dmg"
      : target.platform === "windows"
        ? "nsis"
        : "appimage,deb",
  ];
  // Apple Silicon needs an ad-hoc signature even for local test bundles.
  if (target.platform === "macos" && !signed)
    args.push(
      "--config",
      JSON.stringify({ bundle: { macOS: { signingIdentity: "-" } } }),
    );
  if (verbose) args.push("--verbose");
  args.push("--", "--locked");
  command(root, process.execPath, args, {
    env: { CI: "true", CARGO_TARGET_DIR: targetDirectory },
  });
}

function collect(root, target, targetDirectory, stage, version) {
  const bundle = join(targetDirectory, target.triple, "release/bundle");
  const output = join(stage, target.platform);
  mkdirSync(output, { recursive: true });
  const built = [];
  const candidates = discover(bundle);
  for (const extension of formats[target.platform]) {
    const matches = candidates.filter((path) => path.endsWith(extension));
    if (matches.length !== 1)
      throw new Error(
        `Expected one ${extension} for ${target.triple}, found ${matches.length} in ${bundle}.`,
      );
    const filename = `Port-Authority-${version}-${target.platform}-${target.architecture}${extension}`;
    const destination = join(output, filename);
    cpSync(matches[0], destination, {
      recursive: true,
      errorOnExist: true,
      force: false,
      verbatimSymlinks: true,
    });
    if (fingerprint(matches[0]) !== fingerprint(destination))
      throw new Error(`Artifact copy verification failed: ${destination}`);
    built.push(slash(relative(stage, destination)));
    if (extension === ".app") {
      command(
        root,
        "tar",
        ["-czf", `${destination}.tar.gz`, "-C", output, filename],
        { env: { LC_ALL: "C" } },
      );
      built.push(slash(relative(stage, `${destination}.tar.gz`)));
    }
  }
  return built;
}

export function withReleaseLock(root, operation) {
  mkdirSync(join(root, "builds"), { recursive: true });
  const lock = join(root, "builds/.release.lock");
  try {
    mkdirSync(lock);
  } catch (error) {
    if (error.code === "EEXIST")
      throw new Error(
        `Another release/archive operation holds ${lock}. If interrupted, confirm it has stopped before removing this lock.`,
      );
    throw error;
  }
  try {
    writeFileSync(
      join(lock, "OWNER.json"),
      JSON.stringify({ pid: process.pid, timestamp: new Date().toISOString() }),
    );
    return operation();
  } finally {
    rmSync(lock, { recursive: true });
  }
}

export function release(
  root,
  {
    platform,
    arch,
    signed = false,
    verbose = false,
    now = new Date(),
    validate = validateEnvironment,
    build = buildTarget,
  } = {},
) {
  const plan = selectTargets(platform, arch);
  const version = readVersion(root, { verifyTag: root === repositoryRoot });
  const targetDirectory = validate(root, plan);
  const archive = archiveBuilds(root, { version, now, targetDirectory });
  const stage = join(root, "builds/.staging");
  // Reached only after every existing distributable has a verified archive.
  rmSync(stage, { recursive: true, force: true });
  rmSync(join(root, "dist"), { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  const info = {
    ...metadata(root, version, now),
    status: "building",
    previousArchive: archive ? slash(relative(root, archive)) : null,
    signing: signed
      ? "configured credentials"
      : "local test (macOS ad-hoc; Windows unsigned)",
    targets: [],
    artifacts: [],
  };
  const saveInfo = () =>
    writeFileSync(
      join(stage, "BUILD_INFO.json"),
      `${JSON.stringify(info, null, 2)}\n`,
    );
  saveInfo();
  try {
    for (const target of plan) {
      console.log(
        `\nBuilding ${target.platform} ${target.architecture} (${target.triple})…`,
      );
      rmSync(join(targetDirectory, target.triple, "release/bundle"), {
        recursive: true,
        force: true,
      });
      build(root, target, signed, verbose, targetDirectory);
      const paths = collect(root, target, targetDirectory, stage, version);
      info.targets.push({ ...target, status: "built" });
      info.artifacts.push(
        ...paths.map((path) => ({
          path,
          treeSha256: fingerprint(join(stage, path)),
        })),
      );
      saveInfo();
    }
    const sums = info.artifacts
      .filter(({ path }) => lstatSync(join(stage, path)).isFile())
      .map(
        ({ path }) =>
          `${createHash("sha256")
            .update(readFileSync(join(stage, path)))
            .digest("hex")}  ${path}`,
      )
      .join("\n");
    writeFileSync(join(stage, "SHA256SUMS.txt"), `${sums}\n`);
    info.status = "complete";
    saveInfo();
    // Keep the old current available even during compilation; replace after success.
    const current = join(root, "builds/current");
    const previous = join(root, "builds/.previous");
    rmSync(previous, { recursive: true, force: true });
    if (existsSync(current)) renameSync(current, previous);
    try {
      renameSync(stage, current);
    } catch (error) {
      if (existsSync(previous)) renameSync(previous, current);
      throw error;
    }
    rmSync(previous, { recursive: true, force: true });
  } catch (error) {
    info.status = "failed";
    info.error = error.message;
    saveInfo();
    throw new Error(
      `Release failed. Previous current preserved; partial output: ${stage}. Archive: ${archive ?? "none (first build)"}. ${error.message}`,
    );
  }
  console.log(
    `\nPORT AUTHORITY RELEASE\n\nVersion:  ${version}\nCommit:   ${info.gitCommit ?? "unavailable"}${info.gitDirty ? " (working tree modified)" : ""}\nPrevious: ${archive ?? "none (first build)"}\n`,
  );
  for (const target of info.targets)
    console.log(`✓ ${target.platform} ${target.architecture}`);
  for (const other of Object.keys(targets).filter(
    (name) => name !== plan[0].platform,
  ))
    console.log(
      `Skipped ${other}: requires native host/CI runner (not attempted).`,
    );
  for (const { path } of info.artifacts)
    console.log(join(root, "builds/current", path));
  return { archive, info };
}

function main() {
  const args = process.argv.slice(2);
  const options = {};
  const action = args.shift();
  if (!["release", "archive"].includes(action))
    throw new Error(
      "Usage: node scripts/release.mjs release|archive [--platform macos|windows|linux] [--arch arm64|x64|all] [--signed]",
    );
  while (args.length) {
    const flag = args.shift();
    if (flag === "--signed") options.signed = true;
    else if (flag === "--verbose") options.verbose = true;
    else if (
      ["--platform", "--arch"].includes(flag) &&
      args[0] &&
      !args[0].startsWith("--")
    )
      options[flag.slice(2)] = args.shift();
    else throw new Error(`Unknown or incomplete option: ${flag}`);
  }
  withReleaseLock(repositoryRoot, () => {
    if (action === "release") release(repositoryRoot, options);
    else {
      let targetDirectory = resolve(
        repositoryRoot,
        process.env.CARGO_TARGET_DIR ?? "src-tauri/target",
      );
      // Respect Cargo config when available, but archiving must also work without Rust.
      try {
        targetDirectory = JSON.parse(
          command(
            repositoryRoot,
            "cargo",
            [
              "metadata",
              "--no-deps",
              "--format-version",
              "1",
              "--manifest-path",
              "src-tauri/Cargo.toml",
            ],
            { capture: true },
          ),
        ).target_directory;
      } catch {
        /* Default/environment directory is still scanned. */
      }
      if (!archiveBuilds(repositoryRoot, { targetDirectory }))
        console.log("No existing distributables to archive (first build).");
    }
  });
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    main();
  } catch (error) {
    console.error(`Port Authority: ${error.message}`);
    process.exitCode = 1;
  }
}
