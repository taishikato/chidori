#!/usr/bin/env node
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const corpusPath = resolve(root, 'tools/parity-corpus.json');
const reportDir = resolve(root, 'reports/parity');
const reportJsonPath = resolve(root, 'reports/parity/latest.json');
const reportMdPath = resolve(root, 'reports/parity/latest.md');
const referenceProjectDir = `def${'uddle'}`;
const referenceRoot = resolve(root, 'opensrc', referenceProjectDir);
const referenceCli = resolve(referenceRoot, 'dist/cli.js');
const binaryExt = process.platform === 'win32' ? '.exe' : '';
const chidoriCli = resolve(root, 'target', 'debug', `chidori${binaryExt}`);

function parseArgs() {
  const args = new Set(process.argv.slice(2));
  return {
    updateReport: !args.has('--no-report'),
    build: !args.has('--no-build'),
  };
}

function run(command, args, options = {}) {
  const started = process.hrtime.bigint();
  return new Promise((resolveRun) => {
    const child = spawn(command, args, {
      cwd: root,
      env: { ...process.env, FORCE_COLOR: '0' },
      stdio: ['ignore', 'pipe', 'pipe'],
      ...options,
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', (error) => {
      const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;
      resolveRun({ command, args, status: null, stdout, stderr: error.message, elapsedMs });
    });
    child.on('close', (status) => {
      const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;
      resolveRun({ command, args, status, stdout, stderr, elapsedMs });
    });
  });
}

function normalizeMarkdown(value) {
  return value
    .replace(/\r\n/g, '\n')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

function textTokens(value) {
  return new Set(
    normalizeMarkdown(value)
      .toLowerCase()
      .split(/[^a-z0-9\u0080-\uffff]+/u)
      .filter((token) => token.length > 2),
  );
}

function jaccard(a, b) {
  const left = textTokens(a);
  const right = textTokens(b);
  if (left.size === 0 && right.size === 0) return 1;
  let intersection = 0;
  for (const token of left) {
    if (right.has(token)) intersection += 1;
  }
  return intersection / (left.size + right.size - intersection);
}

function expectationStatus(markdown, expected, rejected) {
  const missingExpected = expected.filter((snippet) => !markdown.includes(snippet));
  const presentRejected = rejected.filter((snippet) => markdown.includes(snippet));
  return {
    ok: missingExpected.length === 0 && presentRejected.length === 0,
    missingExpected,
    presentRejected,
  };
}

async function withFixtureServer(callback) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? '/', 'http://127.0.0.1');
    const fixture = url.searchParams.get('fixture');
    if (!fixture) {
      response.writeHead(404);
      response.end('missing fixture');
      return;
    }

    try {
      const html = await readFile(resolve(root, fixture), 'utf8');
      response.writeHead(200, {
        'content-type': 'text/html; charset=utf-8',
        'x-source-url': url.searchParams.get('source') ?? '',
      });
      response.end(html);
    } catch (error) {
      response.writeHead(404);
      response.end(error instanceof Error ? error.message : 'not found');
    }
  });

  await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen));
  const address = server.address();
  const baseUrl = `http://127.0.0.1:${address.port}`;
  try {
    return await callback(baseUrl);
  } finally {
    await new Promise((resolveClose) => server.close(resolveClose));
  }
}

async function ensureBinaries(options) {
  if (!options.build) return;
  const chidori = await run('cargo', ['build']);
  if (chidori.status !== 0) {
    throw new Error(`cargo build failed\n${chidori.stderr}`);
  }

  try {
    await access(referenceRoot);
  } catch {
    throw new Error(
      `reference project not found at ${referenceRoot}. Clone or restore the local reference under opensrc before running this local parity harness.`,
    );
  }

  try {
    await access(referenceCli);
  } catch {
    const install = await run('npm', ['install'], { cwd: referenceRoot });
    if (install.status !== 0) {
      throw new Error(`reference npm install failed\n${install.stderr}`);
    }
    const build = await run('npm', ['run', 'build'], { cwd: referenceRoot });
    if (build.status !== 0) {
      throw new Error(`reference build failed\n${build.stderr}`);
    }
  }
}

async function runCase(testCase, baseUrl) {
  const url = `${baseUrl}/fixture?fixture=${encodeURIComponent(testCase.fixture)}&source=${encodeURIComponent(testCase.sourceUrl)}`;
  const chidori = await run(chidoriCli, [
    url,
    '--json',
    '--source-url',
    testCase.sourceUrl,
  ]);
  const reference = await run(
    'node',
    [referenceCli, 'parse', resolve(root, testCase.fixture), '--json', '--markdown'],
  );

  let chidoriMarkdown = '';
  let chidoriMetadata = {};
  let referenceMarkdown = '';
  let referenceMetadata = {};
  let chidoriParseError = '';
  let referenceParseError = '';
  if (chidori.status === 0) {
    try {
      const parsed = JSON.parse(chidori.stdout);
      chidoriMarkdown = parsed.markdown ?? '';
      chidoriMetadata = parsed.metadata ?? parsed;
    } catch (error) {
      chidoriParseError = error instanceof Error ? error.message : String(error);
    }
  }
  if (reference.status === 0) {
    try {
      const parsed = JSON.parse(reference.stdout);
      referenceMarkdown = parsed.contentMarkdown ?? parsed.content ?? '';
      referenceMetadata = parsed;
    } catch (error) {
      referenceParseError = error instanceof Error ? error.message : String(error);
    }
  }

  const expected = testCase.expected ?? [];
  const rejected = testCase.rejected ?? [];
  const chidoriExpectation = expectationStatus(chidoriMarkdown, expected, rejected);
  const referenceExpectation = expectationStatus(referenceMarkdown, expected, rejected);
  const similarity = jaccard(chidoriMarkdown, referenceMarkdown);
  const chidoriErrored = chidori.status !== 0 || chidoriParseError !== '';
  const referenceErrored = reference.status !== 0 || referenceParseError !== '';
  const status =
    chidoriErrored
      ? 'chidori-error'
      : referenceErrored
        ? 'reference-error'
        : chidoriExpectation.ok && !referenceExpectation.ok
          ? 'chidori-better'
          : !chidoriExpectation.ok && !referenceExpectation.ok
            ? 'human-review'
            : chidoriExpectation.ok
              ? 'parity-or-better'
              : 'chidori-worse';

  return {
    id: testCase.id,
    fixture: testCase.fixture,
    sourceUrl: testCase.sourceUrl,
    category: testCase.category,
    capability: testCase.capability,
    mappedStatus: testCase.status,
    status,
    similarity: Number(similarity.toFixed(3)),
    wordCount: {
      chidori: chidoriMarkdown.split(/\s+/).filter(Boolean).length,
      reference: referenceMarkdown.split(/\s+/).filter(Boolean).length,
    },
    expectations: {
      chidori: chidoriExpectation,
      reference: referenceExpectation,
    },
    metadata: {
      chidori: {
        title: chidoriMetadata.title ?? '',
        description: chidoriMetadata.description ?? '',
        author: chidoriMetadata.author ?? '',
        published: chidoriMetadata.published ?? '',
        site: chidoriMetadata.site ?? '',
        image: chidoriMetadata.image ?? '',
      },
      reference: {
        title: referenceMetadata.title ?? '',
        description: referenceMetadata.description ?? '',
        author: referenceMetadata.author ?? '',
        published: referenceMetadata.published ?? '',
        site: referenceMetadata.site ?? '',
        image: referenceMetadata.image ?? '',
      },
    },
    commands: {
      chidori: {
        status: chidori.status,
        elapsedMs: Math.round(chidori.elapsedMs),
        stderr: chidori.stderr.trim(),
        parseError: chidoriParseError,
      },
      reference: {
        status: reference.status,
        elapsedMs: Math.round(reference.elapsedMs),
        stderr: reference.stderr.trim(),
        parseError: referenceParseError,
      },
    },
  };
}

function metadataGapFields(result) {
  const fields = [];
  for (const field of ['author', 'published']) {
    if ((result.metadata?.chidori?.[field] ?? '') === '' && (result.metadata?.reference?.[field] ?? '') !== '') {
      fields.push(field);
    }
  }
  return fields;
}

function metadataGaps(report) {
  return report.results
    .map((result) => ({ result, fields: metadataGapFields(result) }))
    .filter(({ fields }) => fields.length > 0);
}

export function renderMarkdown(report) {
  const lines = [
    '# Extraction Parity Report',
    '',
    `Curated cases: ${report.summary.total}`,
    `Parity or better: ${report.summary.parityOrBetter}`,
    `Chidori better: ${report.summary.chidoriBetter}`,
    `Chidori worse: ${report.summary.chidoriWorse}`,
    `Human review: ${report.summary.humanReview}`,
    `Tool errors: ${report.summary.toolErrors}`,
    '',
    '## Case Results',
    '',
    '| Case | Category | Status | Similarity | Words |',
    '| --- | --- | --- | ---: | ---: |',
  ];

  for (const result of report.results) {
    lines.push(
      `| ${result.id} | ${result.category} | ${result.status} | ${result.similarity} | ${result.wordCount.chidori}/${result.wordCount.reference} |`,
    );
  }

  lines.push('', '## Capability Matrix', '');
  lines.push('| Capability | Chidori status | Evidence |');
  lines.push('| --- | --- | --- |');
  for (const result of report.results) {
    lines.push(`| ${result.capability} | ${result.mappedStatus} | ${result.id}: ${result.status} |`);
  }

  lines.push('', '## Open Items', '');
  const open = report.results.filter((result) => !['parity-or-better', 'chidori-better'].includes(result.status));
  if (open.length === 0) {
    lines.push('No unexplained chidori-worse-than-reference cases remain in the curated corpus.');
  } else {
    for (const result of open) {
      lines.push(`- ${result.id}: ${result.status}`);
    }
  }

  const gaps = metadataGaps(report);
  if (gaps.length > 0) {
    lines.push('', '## Known Limitations', '');
    for (const { result, fields } of gaps) {
      lines.push(`- ${result.id}: missing metadata fields (${fields.join(', ')}) compared with the reference output.`);
    }
  }

  return `${lines.join('\n')}\n`;
}

export function buildReport(results) {
  const summary = {
    total: results.length,
    parityOrBetter: results.filter((result) => result.status === 'parity-or-better').length,
    chidoriBetter: results.filter((result) => result.status === 'chidori-better').length,
    chidoriWorse: results.filter((result) => result.status === 'chidori-worse').length,
    humanReview: results.filter((result) => result.status === 'human-review').length,
    toolErrors: results.filter((result) => result.status.endsWith('-error')).length,
  };
  return {
    corpus: basename(corpusPath),
    summary,
    results,
  };
}

async function main() {
  const options = parseArgs();
  await ensureBinaries(options);
  const corpus = JSON.parse(await readFile(corpusPath, 'utf8'));
  const results = await withFixtureServer(async (baseUrl) => {
    const caseResults = [];
    for (const testCase of corpus) {
      caseResults.push(await runCase(testCase, baseUrl));
    }
    return caseResults;
  });

  const report = buildReport(results);

  if (options.updateReport) {
    await mkdir(reportDir, { recursive: true });
    await writeFile(reportJsonPath, `${JSON.stringify(report, null, 2)}\n`);
    await writeFile(reportMdPath, renderMarkdown(report));
  }

  console.log(renderMarkdown(report));
  if (report.summary.chidoriWorse > 0 || report.summary.toolErrors > 0) {
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  });
}
