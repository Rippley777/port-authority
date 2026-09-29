import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
for (const shell of ["bash", "zsh"]) {
  const available =
    process.platform !== "win32" &&
    spawnSync(shell, ["--version"]).status === 0;
  test(
    `${shell}: explicit capture, dev-command interception, bypass, exit status, and uninstall`,
    { skip: !available },
    () => {
      const dir = mkdtempSync(join(tmpdir(), "pa-shell-test-"));
      try {
        const bin = join(dir, "port authority");
        const args = join(dir, "arguments");
        const npm = join(dir, "npm");
        writeFileSync(
          bin,
          '#!/bin/sh\nprintf "%s\\n" "$@" > "$PA_TEST_ARGUMENTS"\nexit 37\n',
          { mode: 0o700 },
        );
        writeFileSync(
          npm,
          '#!/bin/sh\nprintf "%s\\n" bypass > "$PA_TEST_ARGUMENTS"\n',
          { mode: 0o700 },
        );
        const driver = `source "$PA_TEST_SCRIPT"
pa npm run dev 'literal $(must-not-run)'
[ "$?" -eq 37 ] || exit 51
cp "$PA_TEST_ARGUMENTS" "$PA_TEST_ARGUMENTS.explicit"
pa_autopilot_on >/dev/null || exit 52
npm run dev -- --port 5173
[ "$?" -eq 37 ] || exit 53
cp "$PA_TEST_ARGUMENTS" "$PA_TEST_ARGUMENTS.auto"
npm --version || exit 54
[ "$(cat "$PA_TEST_ARGUMENTS")" = bypass ] || exit 55
pa_autopilot_off
npm run dev || exit 56
[ "$(cat "$PA_TEST_ARGUMENTS")" = bypass ] || exit 57
`;
        const result = spawnSync(
          shell,
          shell === "bash"
            ? ["--noprofile", "--norc", "-c", driver]
            : ["-f", "-c", driver],
          {
            encoding: "utf8",
            env: {
              ...process.env,
              PATH: `${dir}:${process.env.PATH}`,
              PORT_AUTHORITY_BIN: bin,
              PA_TEST_ARGUMENTS: args,
              PA_TEST_SCRIPT: resolve("shell/port-authority.sh"),
            },
          },
        );
        assert.equal(result.status, 0, result.stderr);
        assert.equal(
          readFileSync(`${args}.explicit`, "utf8"),
          "--autopilot-run\n--\nnpm\nrun\ndev\nliteral $(must-not-run)\n",
        );
        assert.equal(
          readFileSync(`${args}.auto`, "utf8"),
          "--autopilot-run\n--\nnpm\nrun\ndev\n--\n--port\n5173\n",
        );
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    },
  );
  test(
    `${shell}: refuses to overwrite existing package-manager functions or aliases`,
    { skip: !available },
    () => {
      for (const existing of [
        "function npm { printf preserved; }",
        "alias npm='printf preserved'",
      ]) {
        const driver = `${existing}\nsource "$PA_TEST_SCRIPT"\npa_autopilot_on >/dev/null 2>&1 && exit 61\n(typeset -f npm || alias npm) | command grep preserved >/dev/null || exit 62\n`;
        const result = spawnSync(shell, ["-c", driver], {
          encoding: "utf8",
          env: {
            ...process.env,
            PA_TEST_SCRIPT: resolve("shell/port-authority.sh"),
          },
        });
        assert.equal(result.status, 0, result.stderr);
      }
    },
  );
}
