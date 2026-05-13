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
const platformPackageNames = entries
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("chidori-fetch-"))
  .map((entry) => entry.name)
  .sort();

packageJson.version = version;
packageJson.optionalDependencies = Object.fromEntries(
  platformPackageNames.map((name) => [name, version]),
);
await writeJson(packageJsonPath, packageJson);

for (const name of platformPackageNames) {
  const platformPackageJsonPath = resolve(npmRoot, name, "package.json");
  const platformPackageJson = await readJson(platformPackageJsonPath);
  platformPackageJson.version = version;
  await writeJson(platformPackageJsonPath, platformPackageJson);
}

console.log(`Synchronized npm package versions to ${version}`);
