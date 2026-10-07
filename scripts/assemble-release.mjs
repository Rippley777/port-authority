import {
  readdirSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  cpSync,
  existsSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  repositoryRoot,
  readVersion,
  archiveBuilds,
  withReleaseLock,
} from "./release.mjs";

// Artifact uploads omit raw .app trees because ZIP transport loses modes/symlinks.
// Their portable .app.tar.gz versions are included instead.
export function assemble(root) {
  const version = readVersion(root, { verifyTag: root === repositoryRoot });
  const downloaded = join(root, "builds/downloaded");
  const selected = new Map();
  for (const name of readdirSync(downloaded)) {
    const match = /^release-(macos|windows|linux)-(\d+)$/.exec(name);
    if (!match) continue;
    const attempt = Number(match[2]);
    if (!selected.has(match[1]) || selected.get(match[1]).attempt < attempt)
      selected.set(match[1], { attempt, directory: join(downloaded, name) });
  }
  const manifests = [];
  const artifacts = [];
  for (const platform of ["macos", "windows", "linux"]) {
    const entry = selected.get(platform);
    if (!entry) throw new Error(`Missing ${platform} artifacts.`);
    const info = JSON.parse(
      readFileSync(join(entry.directory, "BUILD_INFO.json"), "utf8"),
    );
    if (
      info.status !== "complete" ||
      info.version !== version ||
      !info.targets.every(
        (target) => target.platform === platform && target.status === "built",
      )
    )
      throw new Error(`Invalid ${platform} build metadata.`);
    const expected = platform === "macos" ? ["arm64", "x64"] : ["x64"];
    if (
      expected.some(
        (arch) => !info.targets.some((target) => target.architecture === arch),
      )
    )
      throw new Error(`Missing ${platform} architecture.`);
    const extensions =
      platform === "macos"
        ? [".dmg", ".app.tar.gz"]
        : platform === "windows"
          ? [".exe"]
          : [".AppImage", ".deb"];
    const expectedPaths = expected.flatMap((arch) =>
      extensions.map(
        (extension) =>
          `${platform}/Port-Authority-${version}-${platform}-${arch}${extension}`,
      ),
    );
    const transported = info.artifacts.filter(
      ({ path }) => !path.endsWith(".app"),
    );
    if (
      transported.length !== expectedPaths.length ||
      expectedPaths.some(
        (path) => !transported.some((artifact) => artifact.path === path),
      )
    )
      throw new Error(`Missing or unexpected ${platform} distributables.`);
    const sums = new Map(
      readFileSync(join(entry.directory, "SHA256SUMS.txt"), "utf8")
        .trim()
        .split("\n")
        .map((line) => {
          const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
          if (!match)
            throw new Error(`Invalid checksum record in ${platform}.`);
          return [match[2], match[1]];
        }),
    );
    for (const { path } of info.artifacts.filter(
      ({ path }) => !path.endsWith(".app"),
    )) {
      if (
        !path.startsWith(`${platform}/`) ||
        path.includes("..") ||
        path.includes("\\") ||
        !/\.(dmg|app\.tar\.gz|exe|AppImage|deb)$/.test(path)
      )
        throw new Error(`Invalid artifact path: ${path}`);
      const sha256 = createHash("sha256")
        .update(readFileSync(join(entry.directory, path)))
        .digest("hex");
      if (sums.get(path) !== sha256)
        throw new Error(`Checksum mismatch: ${path}`);
      artifacts.push({ path, sha256, source: join(entry.directory, path) });
    }
    manifests.push(info);
  }
  if (
    !manifests[0].gitCommit ||
    manifests.some((info) => info.gitCommit !== manifests[0].gitCommit)
  )
    throw new Error(
      "Platform artifacts have different or missing Git commits.",
    );
  if (new Set(artifacts.map(({ path }) => path)).size !== artifacts.length)
    throw new Error("Duplicate artifact paths.");
  archiveBuilds(root, { version });
  const current = join(root, "builds/current");
  // CI is a fresh checkout. Refuse to merge a second set into a populated current.
  if (existsSync(current) && readdirSync(current).length)
    throw new Error(
      "Current is already populated; assemble in a fresh checkout. Existing current was archived.",
    );
  for (const { path, source } of artifacts) {
    mkdirSync(dirname(join(current, path)), { recursive: true });
    cpSync(source, join(current, path), { errorOnExist: true, force: false });
  }
  const info = {
    version,
    timestamp: new Date().toISOString(),
    gitCommit: manifests[0].gitCommit,
    gitBranch: manifests[0].gitBranch,
    gitRef: manifests[0].gitRef,
    status: "complete",
    targets: manifests.flatMap((info) => info.targets),
    platformBuilds: manifests,
    artifacts: artifacts.map(({ source, ...artifact }) => artifact),
  };
  writeFileSync(
    join(current, "BUILD_INFO.json"),
    `${JSON.stringify(info, null, 2)}\n`,
  );
  writeFileSync(
    join(current, "SHA256SUMS.txt"),
    `${artifacts.map(({ path, sha256 }) => `${sha256}  ${path}`).join("\n")}\n`,
  );
  console.log(
    `Verified ${artifacts.length} distributables for Port Authority ${version} (${info.gitCommit}).`,
  );
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    withReleaseLock(repositoryRoot, () => assemble(repositoryRoot));
  } catch (error) {
    console.error(`Port Authority: ${error.message}`);
    process.exitCode = 1;
  }
}
