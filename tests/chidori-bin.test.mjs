import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

function runLauncherScript(script) {
  return spawnSync(process.execPath, ["-e", script], {
    cwd: new URL("..", import.meta.url),
    encoding: "utf8",
  });
}

test("launcher maps supported Node platforms to npm platform package names", () => {
  const result = runLauncherScript(`
    const launcher = require("./bin/chidori.js");
    console.log(JSON.stringify({
      darwinArm64: launcher.platformPackageName("darwin", "arm64"),
      linuxX64: launcher.platformPackageName("linux", "x64"),
      win32Arm64: launcher.platformPackageName("win32", "arm64"),
      unsupported: launcher.platformPackageName("freebsd", "x64") ?? null,
    }));
  `);

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    darwinArm64: "@chidori-fetch/darwin-arm64",
    linuxX64: "@chidori-fetch/linux-x64",
    win32Arm64: "@chidori-fetch/win32-arm64",
    unsupported: null,
  });
});

test("launcher resolves the optional platform package before local cargo build outputs", () => {
  const result = runLauncherScript(`
    const launcher = require("./bin/chidori.js");
    const binary = launcher.resolveBinaryPath({
      root: "/repo",
      platform: "linux",
      arch: "x64",
      resolvePackageBinary: () => "/repo/node_modules/@chidori-fetch/linux-x64/bin/chidori",
      pathExists: () => true,
    });
    console.log(binary);
  `);

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "/repo/node_modules/@chidori-fetch/linux-x64/bin/chidori");
});

test("launcher falls back to local cargo build outputs for repository development", () => {
  const result = runLauncherScript(`
    const launcher = require("./bin/chidori.js");
    const binary = launcher.resolveBinaryPath({
      root: "/repo",
      platform: "darwin",
      arch: "arm64",
      resolvePackageBinary: () => {
        throw Object.assign(new Error("missing package"), { code: "MODULE_NOT_FOUND" });
      },
      pathExists: (candidate) => candidate === "/repo/target/release/chidori",
    });
    console.log(binary);
  `);

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "/repo/target/release/chidori");
});
