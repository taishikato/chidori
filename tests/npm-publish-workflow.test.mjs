import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const workflowPath = new URL("../.github/workflows/npm-publish.yml", import.meta.url);

test("npm publish workflow uses trusted publishing instead of npm tokens", () => {
  assert.equal(existsSync(workflowPath), true, "missing npm publish workflow");

  const workflow = readFileSync(workflowPath, "utf8");

  assert.match(workflow, /^name:\s*Publish npm package$/m);
  assert.match(workflow, /^\s*workflow_dispatch:/m);
  assert.match(workflow, /^\s*id-token:\s*write$/m);
  assert.match(workflow, /^\s*contents:\s*read$/m);
  assert.match(workflow, /node-version:\s*"24"/);
  assert.match(
    workflow,
    /actions\/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd/,
  );
  assert.match(
    workflow,
    /actions\/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e/,
  );
  assert.match(workflow, /npm install -g npm@11\.10\.0/);
  assert.doesNotMatch(workflow, /uses:\s*actions\/(?:checkout|setup-node)@v\d+/);
  assert.doesNotMatch(workflow, /npm install -g npm@latest/);
  assert.match(workflow, /npm pack --dry-run/);
  assert.match(workflow, /chidori-fetch-darwin-arm64/);
  assert.match(workflow, /chidori-fetch-darwin-x64/);
  assert.match(workflow, /chidori-fetch-linux-arm64/);
  assert.match(workflow, /chidori-fetch-linux-x64/);
  assert.match(workflow, /chidori-fetch-win32-arm64/);
  assert.match(workflow, /chidori-fetch-win32-x64/);
  assert.match(workflow, /node tools\/sync-npm-platform-version\.mjs "\$DRY_RUN_VERSION"/);
  assert.match(
    workflow,
    /node tools\/prepare-platform-package\.mjs --package "\$\{\{ matrix\.package \}\}" --binary "target\/release\/\$\{\{ matrix\.binary \}\}"/,
  );
  assert.match(workflow, /npm publish "\.\/npm\/\$\{\{ matrix\.package \}\}" --dry-run --tag dry-run/);
  assert.match(workflow, /npm version "\$DRY_RUN_VERSION" --no-git-tag-version/);
  assert.match(workflow, /npm publish --dry-run --tag dry-run/);
  assert.match(workflow, /npm publish$/m);
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/);
});
