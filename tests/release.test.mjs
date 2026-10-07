import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  existsSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import {
  archiveBuilds,
  fingerprint,
  release,
  readVersion,
  selectTargets,
  withReleaseLock,
} from "../scripts/release.mjs";

const now = new Date("2026-10-03T00:42:31Z");
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "port authority release "));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  put(root, "package.json", JSON.stringify({ version: "0.1.0" }));
  put(root, "src-tauri/tauri.conf.json", JSON.stringify({ version: "0.1.0" }));
  put(
    root,
    "src-tauri/Cargo.toml",
    '[package]\nname = "test"\nversion = "0.1.0"\n',
  );
  return root;
}
function put(root, path, contents = "artifact") {
  mkdirSync(join(root, path, ".."), { recursive: true });
  writeFileSync(join(root, path), contents);
}
function load(root, path) {
  return JSON.parse(readFileSync(join(root, path), "utf8"));
}

test("first archive tolerates missing directories and unavailable Git", (t) => {
  const root = fixture(t);
  assert.equal(archiveBuilds(root, { now }), null);
  put(root, "artifacts/beta/Port Authority.zip");
  const archive = archiveBuilds(root, { now });
  const info = load(archive, "BUILD_INFO.json");
  assert.equal(info.status, "complete");
  assert.equal(info.gitCommit, null);
  assert.equal(info.gitBranch, null);
});

test("archives current, legacy and packaged apps, excludes intermediates; same timestamp never overwrites", (t) => {
  const root = fixture(t);
  put(root, "builds/current/macos/old.dmg", "current");
  put(root, "builds/current/BUILD_INFO.json", '{"version":"0.0.9"}');
  put(
    root,
    "src-tauri/target/release/bundle/macos/Port Authority.app/Contents/MacOS/port-authority",
  );
  put(
    root,
    "src-tauri/target/universal-apple-darwin/release/bundle/dmg/old.dmg",
  );
  put(
    root,
    "src-tauri/target/debug/bundle/macos/Debug.app/Contents/MacOS/port-authority",
  );
  put(root, "src-tauri/target/debug/deps/intermediate");
  put(root, "src-tauri/target/release/bundle/macos/rw.123.scratch.dmg");
  put(root, "node_modules/cache.zip");
  const first = archiveBuilds(root, { now });
  const second = archiveBuilds(root, { now });
  assert.notEqual(first, second);
  assert.equal(second, `${first}-001`);
  assert.equal(load(first, "PREVIOUS_BUILD_INFO.json").version, "0.0.9");
  const info = load(first, "BUILD_INFO.json");
  assert.equal(info.artifacts.length, 4);
  assert.equal(readFileSync(join(first, "macos/old.dmg"), "utf8"), "current");
  for (const record of info.artifacts) {
    assert.equal(
      fingerprint(join(root, record.source)),
      fingerprint(join(first, record.path)),
    );
    assert.equal(record.treeSha256, fingerprint(join(first, record.path)));
  }
  assert.ok(existsSync(join(root, "builds/current/macos/old.dmg")));
});

test("platform/version validation is actionable and happens without cleanup", (t) => {
  const root = fixture(t);
  assert.throws(
    () => selectTargets("windows", "x64", "macos"),
    /native windows/,
  );
  assert.throws(
    () => selectTargets("linux", "arm64", "linux"),
    /not configured/,
  );
  assert.equal(selectTargets("macos", "all", "macos").length, 2);
  put(root, "package.json", '{"version":"1.0.0"}');
  assert.throws(() => readVersion(root), /Version mismatch/);
});

test("release lock excludes concurrent operations and is released on failure", (t) => {
  const root = fixture(t);
  assert.throws(
    () =>
      withReleaseLock(root, () => {
        assert.throws(() => withReleaseLock(root, () => {}), /Another release/);
        throw new Error("test failure");
      }),
    /test failure/,
  );
  assert.ok(!existsSync(join(root, "builds/.release.lock")));
});

test("failed archive cannot clean current, dist or framework output", (t) => {
  const root = fixture(t);
  const platform = { darwin: "macos", win32: "windows", linux: "linux" }[
    process.platform
  ];
  put(root, "builds/current/old.dmg");
  put(root, "dist/index.html", "old frontend");
  put(root, "builds/archive", "not a directory");
  let built = false;
  assert.throws(() =>
    release(root, {
      platform,
      arch: "x64",
      now,
      validate: () => join(root, "src-tauri/target"),
      build: () => {
        built = true;
      },
    }),
  );
  assert.equal(built, false);
  assert.equal(
    readFileSync(join(root, "dist/index.html"), "utf8"),
    "old frontend",
  );
  assert.ok(existsSync(join(root, "builds/current/old.dmg")));
});

test(
  "actual copy failure retains original artifact and partial archive",
  { skip: process.platform === "win32" },
  (t) => {
    const root = fixture(t);
    put(root, "builds/current/a.dmg");
    put(root, "builds/current/pipe.app/Contents/ok");
    const fifo = join(root, "builds/current/pipe.app/Contents/unsupported");
    assert.equal(spawnSync("mkfifo", [fifo]).status, 0);
    assert.throws(
      () => archiveBuilds(root, { now }),
      /Archival failed; no build output was cleaned/,
    );
    assert.ok(existsSync(join(root, "builds/current/a.dmg")));
    const archives = readdirSync(join(root, "builds/archive"));
    assert.equal(archives.length, 1);
    assert.equal(
      load(root, `builds/archive/${archives[0]}/BUILD_INFO.json`).status,
      "archiving",
    );
  },
);

test("two releases archive before builder, normalize output, replace stale current and keep Git metadata", (t) => {
  const root = fixture(t);
  const platform = { darwin: "macos", win32: "windows", linux: "linux" }[
    process.platform
  ];
  const targetDirectory = join(root, "src-tauri/target");
  const git = (args) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  git(["init", "-b", "release-test"]);
  git(["add", "."]);
  git([
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.test",
    "commit",
    "-m",
    "fixture",
  ]);
  const commit = git(["rev-parse", "HEAD"]);
  put(root, "builds/current/old.dmg", "previous");
  const oldHash = fingerprint(join(root, "builds/current/old.dmg"));
  let count = 0;
  const build = (_, target) => {
    count++;
    assert.equal(readdirSync(join(root, "builds/archive")).length, count);
    if (count === 1)
      assert.equal(fingerprint(join(root, "builds/current/old.dmg")), oldHash);
    const base = `src-tauri/target/${target.triple}/release/bundle`;
    if (platform === "macos") {
      put(
        root,
        `${base}/macos/Port Authority.app/Contents/MacOS/port-authority`,
        `build ${count}`,
      );
      put(root, `${base}/dmg/Port Authority.dmg`, `build ${count}`);
      symlinkSync(
        "MacOS/port-authority",
        join(root, base, "macos/Port Authority.app/Contents/link"),
      );
    } else if (platform === "windows")
      put(root, `${base}/nsis/setup.exe`, `build ${count}`);
    else {
      put(root, `${base}/appimage/Port Authority.AppImage`, `build ${count}`);
      put(root, `${base}/deb/Port Authority.deb`, `build ${count}`);
    }
  };
  const options = {
    platform,
    arch: "x64",
    now,
    validate: () => targetDirectory,
    build,
  };
  const first = release(root, options);
  const firstMetadata = readFileSync(
    join(root, "builds/current/BUILD_INFO.json"),
    "utf8",
  );
  const second = release(root, options);
  assert.notEqual(first.archive, second.archive);
  assert.equal(
    readFileSync(join(second.archive, "PREVIOUS_BUILD_INFO.json"), "utf8"),
    firstMetadata,
  );
  const info = load(root, "builds/current/BUILD_INFO.json");
  assert.equal(info.version, "0.1.0");
  assert.equal(info.gitCommit, commit);
  assert.equal(info.gitBranch, "release-test");
  assert.equal(info.status, "complete");
  assert.ok(!existsSync(join(root, "builds/current/old.dmg")));
  assert.ok(
    info.artifacts.every(({ path }) =>
      path.includes(`Port-Authority-0.1.0-${platform}-x64`),
    ),
  );
  assert.ok(
    info.artifacts.every(
      ({ path, treeSha256 }) =>
        fingerprint(join(root, "builds/current", path)) === treeSha256,
    ),
  );
  assert.ok(existsSync(join(root, "builds/current/SHA256SUMS.txt")));
  if (platform === "macos") {
    const tgz = info.artifacts.find(({ path }) =>
      path.endsWith(".app.tar.gz"),
    ).path;
    assert.equal(
      spawnSync("tar", ["-tzf", join(root, "builds/current", tgz)]).status,
      0,
    );
  }
});

test("failed build preserves current and records partial output for next archive", (t) => {
  const root = fixture(t);
  put(root, "builds/current/old.dmg", "previous");
  const platform = { darwin: "macos", win32: "windows", linux: "linux" }[
    process.platform
  ];
  assert.throws(
    () =>
      release(root, {
        platform,
        arch: "x64",
        now,
        validate: () => join(root, "src-tauri/target"),
        build: () => {
          throw new Error("compiler failed");
        },
      }),
    /Previous current preserved/,
  );
  assert.equal(
    readFileSync(join(root, "builds/current/old.dmg"), "utf8"),
    "previous",
  );
  assert.equal(load(root, "builds/.staging/BUILD_INFO.json").status, "failed");
});

test("CI assembly verifies checksums/commit, selects latest rerun and includes every platform", async (t) => {
  const { assemble } = await import("../scripts/assemble-release.mjs");
  const { createHash } = await import("node:crypto");
  const root = fixture(t);
  const makePlatform = (platform, attempt, commit = "abc123") => {
    const directory = `builds/downloaded/release-${platform}-${attempt}`;
    const architectures = platform === "macos" ? ["arm64", "x64"] : ["x64"];
    const extensions =
      platform === "macos"
        ? [".dmg", ".app.tar.gz"]
        : platform === "windows"
          ? [".exe"]
          : [".AppImage", ".deb"];
    const paths = architectures.flatMap((arch) =>
      extensions.map(
        (ext) => `${platform}/Port-Authority-0.1.0-${platform}-${arch}${ext}`,
      ),
    );
    const contents = `attempt ${attempt}`;
    const sha = createHash("sha256").update(contents).digest("hex");
    for (const path of paths) put(root, `${directory}/${path}`, contents);
    put(
      root,
      `${directory}/BUILD_INFO.json`,
      JSON.stringify({
        version: "0.1.0",
        status: "complete",
        gitCommit: commit,
        gitBranch: "main",
        targets: architectures.map((architecture) => ({
          platform,
          architecture,
          status: "built",
        })),
        artifacts: paths.map((path) => ({ path })),
      }),
    );
    put(
      root,
      `${directory}/SHA256SUMS.txt`,
      paths.map((path) => `${sha}  ${path}`).join("\n"),
    );
  };
  makePlatform("macos", 1);
  makePlatform("macos", 2);
  makePlatform("windows", 1);
  makePlatform("linux", 1);
  const windowsArtifact =
    "builds/downloaded/release-windows-1/windows/Port-Authority-0.1.0-windows-x64.exe";
  put(root, windowsArtifact, "corrupted");
  assert.throws(() => assemble(root), /Checksum mismatch/);
  assert.ok(!existsSync(join(root, "builds/current")));
  makePlatform("windows", 1, "different");
  assert.throws(() => assemble(root), /different or missing Git commits/);
  makePlatform("windows", 1);
  assemble(root);
  const info = load(root, "builds/current/BUILD_INFO.json");
  assert.equal(info.artifacts.length, 7);
  assert.equal(info.targets.length, 4);
  assert.equal(
    readFileSync(
      join(root, "builds/current/macos/Port-Authority-0.1.0-macos-arm64.dmg"),
      "utf8",
    ),
    "attempt 2",
  );
  assert.throws(() => assemble(root), /already populated/);
  assert.equal(readdirSync(join(root, "builds/archive")).length, 1);
});

test("custom external Cargo outputs and default outputs are both archived", (t) => {
  const root = fixture(t);
  const targetDirectory = mkdtempSync(
    join(tmpdir(), "port authority cargo target "),
  );
  t.after(() => rmSync(targetDirectory, { recursive: true, force: true }));
  put(targetDirectory, "x86_64-apple-darwin/release/bundle/dmg/Custom.dmg");
  put(root, "src-tauri/target/release/bundle/dmg/Default.dmg");
  const archive = archiveBuilds(root, { now, targetDirectory });
  const info = load(archive, "BUILD_INFO.json");
  assert.equal(info.artifacts.length, 2);
  assert.ok(
    info.artifacts.some(({ path }) =>
      path.startsWith("macos/legacy/cargo-target/"),
    ),
  );
  for (const record of info.artifacts)
    assert.equal(fingerprint(join(archive, record.path)), record.treeSha256);
});

test(
  "top-level artifact symlinks fail safely instead of creating dependent archives",
  { skip: process.platform === "win32" },
  (t) => {
    const root = fixture(t);
    put(root, "outside.zip");
    mkdirSync(join(root, "builds/current"), { recursive: true });
    symlinkSync("../../outside.zip", join(root, "builds/current/alias.zip"));
    assert.throws(() => archiveBuilds(root, { now }), /not a symlink/);
    assert.ok(existsSync(join(root, "outside.zip")));
  },
);
