#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join, dirname } = require("node:path");

const root = dirname(__dirname);
const binaryName = process.platform === "win32" ? "chidori.exe" : "chidori";
const candidates = [
  join(root, "target", "release", binaryName),
  join(root, "target", "debug", binaryName)
];
const binary = candidates.find(existsSync);

if (!binary) {
  console.error("Error: chidori binary not found. Run `npm run build` first.");
  process.exit(8);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`Error: ${result.error.message}`);
  process.exit(8);
}
process.exit(result.status ?? 1);
