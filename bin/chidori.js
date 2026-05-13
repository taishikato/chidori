#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join, dirname } = require("node:path");

const root = dirname(__dirname);
const platformPackages = new Map([
  ["darwin-arm64", "@chidori-fetch/darwin-arm64"],
  ["darwin-x64", "@chidori-fetch/darwin-x64"],
  ["linux-arm64", "@chidori-fetch/linux-arm64"],
  ["linux-x64", "@chidori-fetch/linux-x64"],
  ["win32-arm64", "@chidori-fetch/win32-arm64"],
  ["win32-x64", "@chidori-fetch/win32-x64"],
]);

function binaryFileName(platform = process.platform) {
  return platform === "win32" ? "chidori.exe" : "chidori";
}

function platformPackageName(platform = process.platform, arch = process.arch) {
  return platformPackages.get(`${platform}-${arch}`);
}

function defaultResolvePackageBinary(packageName, binaryName, packageRoot) {
  return require.resolve(`${packageName}/bin/${binaryName}`, { paths: [packageRoot] });
}

function localBuildCandidates(packageRoot, platform = process.platform) {
  const binaryName = binaryFileName(platform);
  return [
    join(packageRoot, "target", "release", binaryName),
    join(packageRoot, "target", "debug", binaryName),
  ];
}

function resolveBinaryPath({
  root: packageRoot = root,
  platform = process.platform,
  arch = process.arch,
  pathExists = existsSync,
  resolvePackageBinary = defaultResolvePackageBinary,
} = {}) {
  const binaryName = binaryFileName(platform);
  const packageName = platformPackageName(platform, arch);

  if (packageName) {
    try {
      return resolvePackageBinary(packageName, binaryName, packageRoot);
    } catch (error) {
      if (error && error.code !== "MODULE_NOT_FOUND") {
        throw error;
      }
    }
  }

  return localBuildCandidates(packageRoot, platform).find(pathExists) ?? null;
}

function missingBinaryMessage(platform = process.platform, arch = process.arch) {
  const packageName = platformPackageName(platform, arch);
  if (!packageName) {
    return `Error: chidori has no prebuilt binary for ${platform}-${arch}.`;
  }

  return [
    `Error: chidori binary not found for ${platform}-${arch}.`,
    `Expected optional package ${packageName} to be installed.`,
    "Reinstall with optional dependencies enabled, or run `npm run build` in the repository for local development.",
  ].join(" ");
}

function main() {
  const binary = resolveBinaryPath();

  if (!binary) {
    console.error(missingBinaryMessage());
    process.exit(8);
  }

  const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    console.error(`Error: ${result.error.message}`);
    process.exit(8);
  }
  if (result.signal && result.status === null) {
    console.error(`Error: chidori terminated by signal ${result.signal}`);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

if (require.main === module) {
  main();
}

module.exports = {
  binaryFileName,
  localBuildCandidates,
  missingBinaryMessage,
  platformPackageName,
  resolveBinaryPath,
};
