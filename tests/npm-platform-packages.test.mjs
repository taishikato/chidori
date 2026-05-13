import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const rootPackagePath = new URL("../package.json", import.meta.url);
const platformRoot = new URL("../npm/", import.meta.url);

const platforms = [
  { name: "@chidori-fetch/darwin-arm64", path: "darwin-arm64", os: "darwin", cpu: "arm64", binary: "chidori" },
  { name: "@chidori-fetch/darwin-x64", path: "darwin-x64", os: "darwin", cpu: "x64", binary: "chidori" },
  { name: "@chidori-fetch/linux-arm64", path: "linux-arm64", os: "linux", cpu: "arm64", binary: "chidori" },
  { name: "@chidori-fetch/linux-x64", path: "linux-x64", os: "linux", cpu: "x64", binary: "chidori" },
  { name: "@chidori-fetch/win32-arm64", path: "win32-arm64", os: "win32", cpu: "arm64", binary: "chidori.exe" },
  { name: "@chidori-fetch/win32-x64", path: "win32-x64", os: "win32", cpu: "x64", binary: "chidori.exe" },
];

function readJson(url) {
  return JSON.parse(readFileSync(url, "utf8"));
}

test("root npm package delegates native binaries to optional platform packages", () => {
  const packageJson = readJson(rootPackagePath);
  const optionalDependencies = packageJson.optionalDependencies ?? {};

  assert.equal(packageJson.scripts.install, undefined);
  assert.equal(packageJson.scripts.prepack, undefined);
  assert.deepEqual(packageJson.files, ["bin", "README.md"]);

  assert.deepEqual(Object.keys(optionalDependencies).sort(), platforms.map(({ name }) => name).sort());
  for (const platform of platforms) {
    assert.equal(optionalDependencies[platform.name], packageJson.version);
  }
});

test("platform package manifests are publishable OS and CPU filtered packages", () => {
  const rootPackage = readJson(rootPackagePath);

  for (const platform of platforms) {
    const packagePath = new URL(`${platform.path}/package.json`, platformRoot);

    assert.equal(existsSync(packagePath), true, `missing ${platform.name}/package.json`);

    const packageJson = readJson(packagePath);
    assert.equal(packageJson.name, platform.name);
    assert.equal(packageJson.version, rootPackage.version);
    assert.equal(packageJson.license, rootPackage.license);
    assert.deepEqual(packageJson.os, [platform.os]);
    assert.deepEqual(packageJson.cpu, [platform.cpu]);
    assert.deepEqual(packageJson.files, ["bin"]);
    assert.deepEqual(packageJson.bin, { chidori: `bin/${platform.binary}` });
  }
});
