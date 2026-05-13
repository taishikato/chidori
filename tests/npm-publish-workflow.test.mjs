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
  assert.match(workflow, /npm install -g npm@latest/);
  assert.match(workflow, /npm pack --dry-run/);
  assert.match(workflow, /npm publish --dry-run/);
  assert.match(workflow, /npm publish$/m);
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/);
});
