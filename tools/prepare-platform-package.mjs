#!/usr/bin/env node
import { chmod, copyFile, mkdir, readFile, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function valueAfter(name) {
  const index = process.argv.indexOf(name);
  if (index < 0) return undefined;
  return process.argv[index + 1];
}

const packageName = valueAfter("--package");
const binary = valueAfter("--binary");

if (!packageName || !binary) {
  console.error("Usage: prepare-platform-package.mjs --package NAME --binary PATH");
  process.exit(2);
}

const packageRoot = resolve(root, "npm", packageName);
const packageJsonPath = resolve(packageRoot, "package.json");
const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));

if (packageJson.name !== packageName) {
  console.error(`Expected ${packageJsonPath} to declare ${packageName}, found ${packageJson.name}`);
  process.exit(1);
}

const binPath = packageJson.bin?.chidori;
if (!binPath) {
  console.error(`${packageJsonPath} must declare bin.chidori`);
  process.exit(1);
}

const source = resolve(root, binary);
const destination = resolve(packageRoot, binPath);
const sourceStat = await stat(source);

if (!sourceStat.isFile()) {
  console.error(`Binary source is not a file: ${source}`);
  process.exit(1);
}

await mkdir(dirname(destination), { recursive: true });
await copyFile(source, destination);
await chmod(destination, 0o755);

console.log(`Prepared ${packageName} with ${destination}`);
