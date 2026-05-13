import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const workflowPath = new URL("../.github/workflows/npm-publish.yml", import.meta.url);

test("npm publish workflow publishes scoped platform packages with trusted publishing", () => {
  assert.equal(existsSync(workflowPath), true, "missing npm publish workflow");

  const workflow = readFileSync(workflowPath, "utf8");

  assert.match(workflow, /^name:\s*Publish npm package$/m);
  assert.match(workflow, /^\s*workflow_dispatch:/m);
  assert.match(workflow, /^\s*push:/m);
  assert.match(workflow, /^\s*tags:/m);
  assert.match(workflow, /-\s*"v\*\.\*\.\*"/);
  assert.match(workflow, /^\s*id-token:\s*write$/m);
  assert.match(workflow, /^\s*contents:\s*read$/m);
  assert.match(workflow, /dry_run:\s*\$\{\{ steps\.release-mode\.outputs\.dry_run \}\}/);
  assert.match(workflow, /publish:\s*\$\{\{ steps\.release-mode\.outputs\.publish \}\}/);
  assert.match(workflow, /Manual publish is disabled/);
  assert.match(workflow, /GITHUB_ACTOR" != "taishikato"/);
  assert.match(workflow, /Only taishikato can publish/);
  assert.match(workflow, /TAG_VERSION="\$\{GITHUB_REF_NAME#v\}"/);
  assert.match(workflow, /Tag version \$TAG_VERSION does not match package\.json \$PACKAGE_VERSION/);
  assert.match(workflow, /git fetch --no-tags --depth=1 origin main/);
  assert.match(workflow, /git merge-base --is-ancestor HEAD origin\/main/);
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
  assert.match(workflow, /@chidori-fetch\/darwin-arm64/);
  assert.match(workflow, /@chidori-fetch\/darwin-x64/);
  assert.match(workflow, /@chidori-fetch\/linux-arm64/);
  assert.match(workflow, /@chidori-fetch\/linux-x64/);
  assert.match(workflow, /@chidori-fetch\/win32-arm64/);
  assert.match(workflow, /@chidori-fetch\/win32-x64/);
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN/);
  assert.match(workflow, /node tools\/sync-npm-platform-version\.mjs "\$DRY_RUN_VERSION"/);
  assert.match(
    workflow,
    /node tools\/prepare-platform-package\.mjs --package "\$\{\{ matrix\.package \}\}" --directory "\$\{\{ matrix\.path \}\}" --binary "target\/release\/\$\{\{ matrix\.binary \}\}"/,
  );
  assert.match(workflow, /if:\s*\$\{\{ needs\.verify\.outputs\.dry_run == 'true' \}\}/);
  assert.match(workflow, /if:\s*\$\{\{ needs\.verify\.outputs\.publish == 'true' \}\}/);
  assert.match(workflow, /npm publish "\.\/npm\/\$\{\{ matrix\.path \}\}" --dry-run --tag dry-run --access public/);
  assert.match(workflow, /npm publish "\.\/npm\/\$\{\{ matrix\.path \}\}" --access public/);
  assert.match(workflow, /npm version "\$DRY_RUN_VERSION" --no-git-tag-version/);
  assert.match(workflow, /npm publish --dry-run --tag dry-run/);
  assert.match(workflow, /npm publish$/m);
});
