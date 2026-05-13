#!/usr/bin/env node
import { readdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const version = process.argv[2];

if (!version) {
  console.error("Usage: sync-npm-platform-version.mjs VERSION");
  process.exit(2);
}

const packageJsonPath = resolve(root, "package.json");
const npmRoot = resolve(root, "npm");

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

const packageJson = await readJson(packageJsonPath);
const entries = await readdir(npmRoot, { withFileTypes: true });
const platformPackages = [];

for (const entry of entries) {
  if (!entry.isDirectory()) {
    continue;
  }

  const platformPackageJsonPath = resolve(npmRoot, entry.name, "package.json");
  const platformPackageJson = await readJson(platformPackageJsonPath);
  if (platformPackageJson.name?.startsWith("@chidori-fetch/")) {
    platformPackages.push({ directory: entry.name, packageJson: platformPackageJson });
  }
}

platformPackages.sort((left, right) => left.packageJson.name.localeCompare(right.packageJson.name));

packageJson.version = version;
packageJson.optionalDependencies = Object.fromEntries(
  platformPackages.map(({ packageJson }) => [packageJson.name, version]),
);
await writeJson(packageJsonPath, packageJson);

for (const { directory, packageJson } of platformPackages) {
  const platformPackageJsonPath = resolve(npmRoot, directory, "package.json");
  packageJson.version = version;
  await writeJson(platformPackageJsonPath, packageJson);
}

console.log(`Synchronized npm package versions to ${version}`);
