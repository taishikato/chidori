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
  const argv = process.argv.slice(2);
  const args = new Set(argv);
  const valueAfter = (name) => {
    const index = argv.indexOf(name);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  const cases = valueAfter('--case')
    ?.split(',')
    .map((value) => value.trim())
    .filter(Boolean) ?? [];
  return {
    updateReport: !args.has('--no-report'),
    build: !args.has('--no-build'),
    jsonOnly: args.has('--json'),
    cases,
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

function wordCount(value) {
  return normalizeMarkdown(value).match(/[\p{L}\p{N}]+(?:'[\p{L}\p{N}]+)?/gu)?.length ?? 0;
}

function metadataMismatches(actual, expected = {}) {
  return Object.entries(expected)
    .map(([field, expectedValue]) => ({
      field,
      expected: expectedValue,
      actual: actual?.[field] ?? '',
    }))
    .filter(({ expected, actual }) => String(actual) !== String(expected));
}

function wordCountStatus(markdown, range = {}) {
  const words = wordCount(markdown);
  const min = range.min ?? 0;
  const max = range.max ?? Number.POSITIVE_INFINITY;
  return {
    ok: words >= min && words <= max,
    actual: words,
    min,
    max: Number.isFinite(max) ? max : null,
  };
}

function noiseRatioStatus(markdown, noiseSnippets = [], maxNoiseRatio = 0) {
  const normalized = normalizeMarkdown(markdown);
  if (normalized.length === 0) {
    return { ok: true, ratio: 0, matched: [] };
  }
  const matches = noiseSnippets
    .map((snippet) => {
      const normalizedSnippet = normalizeMarkdown(snippet);
      if (normalizedSnippet.length === 0) {
        return { snippet, occurrences: 0, chars: 0 };
      }

      let occurrences = 0;
      let index = normalized.indexOf(normalizedSnippet);
      while (index !== -1) {
        occurrences += 1;
        index = normalized.indexOf(normalizedSnippet, index + normalizedSnippet.length);
      }

      return {
        snippet,
        occurrences,
        chars: occurrences * normalizedSnippet.length,
      };
    })
    .filter(({ occurrences }) => occurrences > 0);
  const matched = matches.map(({ snippet }) => snippet);
  const noiseChars = matches.reduce((sum, { chars }) => sum + chars, 0);
  const ratio = noiseChars / normalized.length;
  return {
    ok: ratio <= maxNoiseRatio,
    ratio: Number(ratio.toFixed(3)),
    matched,
  };
}

function looksLikeExpectation(value) {
  return (
    value !== null
    && typeof value === 'object'
    && [
      'contains',
      'expected',
      'excludes',
      'rejected',
      'metadata',
      'wordCount',
      'noise',
      'maxNoiseRatio',
    ].some((field) => Object.hasOwn(value, field))
  );
}

function normalizeExpectation(expectedOrContains, rejected = []) {
  if (Array.isArray(expectedOrContains)) {
    return {
      contains: expectedOrContains,
      excludes: rejected,
      metadata: {},
      wordCount: {},
      noise: [],
      maxNoiseRatio: 0,
    };
  }

  return {
    contains: expectedOrContains?.contains ?? expectedOrContains?.expected ?? [],
    excludes: expectedOrContains?.excludes ?? expectedOrContains?.rejected ?? [],
    metadata: expectedOrContains?.metadata ?? {},
    wordCount: expectedOrContains?.wordCount ?? {},
    noise: expectedOrContains?.noise ?? [],
    maxNoiseRatio: expectedOrContains?.maxNoiseRatio ?? 0,
  };
}

function expectationStatus(markdown, metadataOrExpected, expectationOrRejected, maybeRejected) {
  const legacyCall = Array.isArray(metadataOrExpected);
  const expectationOnlyCall =
    !legacyCall && looksLikeExpectation(metadataOrExpected) && expectationOrRejected === undefined;
  const metadata = legacyCall || expectationOnlyCall ? {} : metadataOrExpected ?? {};
  const expectation = legacyCall
    ? normalizeExpectation(metadataOrExpected, expectationOrRejected ?? [])
    : expectationOnlyCall
      ? normalizeExpectation(metadataOrExpected)
      : normalizeExpectation(expectationOrRejected ?? {}, maybeRejected ?? []);
  const missingExpected = expectation.contains.filter((snippet) => !markdown.includes(snippet));
  const presentRejected = expectation.excludes.filter((snippet) => markdown.includes(snippet));
  const metadataProblems = metadataMismatches(metadata, expectation.metadata);
  const words = wordCountStatus(markdown, expectation.wordCount);
  const noise = noiseRatioStatus(markdown, expectation.noise, expectation.maxNoiseRatio);

  return {
    ok:
      missingExpected.length === 0
      && presentRejected.length === 0
      && metadataProblems.length === 0
      && words.ok
      && noise.ok,
    missingExpected,
    presentRejected,
    metadataMismatches: metadataProblems,
    wordCount: words,
    noiseRatio: noise,
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

  const expectation = normalizeExpectation({
    contains: testCase.contains ?? testCase.expected ?? [],
    excludes: testCase.excludes ?? testCase.rejected ?? [],
    metadata: testCase.metadata ?? {},
    wordCount: testCase.wordCount ?? {},
    noise: testCase.noise ?? [],
    maxNoiseRatio: testCase.maxNoiseRatio ?? 0,
  });
  const chidoriExpectation = expectationStatus(chidoriMarkdown, chidoriMetadata, expectation);
  const referenceExpectation = expectationStatus(referenceMarkdown, referenceMetadata, expectation);
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

function markdownCode(value) {
  const text = String(value).replace(/\r\n?/g, '\n');
  if (!text.includes('`') && !text.includes('\n') && text.length <= 120) {
    return `\`${text}\``;
  }

  const longestFence =
    text
      .match(/`+/g)
      ?.reduce((max, run) => Math.max(max, run.length), 0) ?? 0;
  const fence = '`'.repeat(Math.max(3, longestFence + 1));
  return `${fence}\n${text}\n${fence}`;
}

function indentedCodeBlock(value) {
  return markdownCode(value)
    .split('\n')
    .map((line) => `    ${line}`)
    .join('\n');
}

function snippetList(label, snippets) {
  if (!snippets || snippets.length === 0) return [];

  const rendered = snippets.map(markdownCode);
  if (rendered.every((snippet) => !snippet.includes('\n'))) {
    return [`${label}: ${rendered.join(', ')}`];
  }

  return snippets.map((snippet) => {
    const code = markdownCode(snippet);
    if (!code.includes('\n')) {
      return `${label}: ${code}`;
    }
    return `${label}:\n${indentedCodeBlock(snippet)}`;
  });
}

function qualityGateLines(result) {
  const status = result.expectations?.chidori;
  if (!status || status.ok) return [];

  const lines = [];
  lines.push(`- ${result.id}:`);
  for (const detail of snippetList('missing', status.missingExpected)) {
    lines.push(`  - ${detail}`);
  }
  for (const detail of snippetList('rejected present', status.presentRejected)) {
    lines.push(`  - ${detail}`);
  }
  for (const mismatch of status.metadataMismatches ?? []) {
    const expected = markdownCode(mismatch.expected);
    const actual = markdownCode(mismatch.actual);
    if (!expected.includes('\n') && !actual.includes('\n')) {
      lines.push(`  - metadata ${mismatch.field}: expected ${expected}, got ${actual}`);
    } else {
      lines.push(`  - metadata ${mismatch.field}:`);
      lines.push(`    expected:\n${indentedCodeBlock(mismatch.expected)}`);
      lines.push(`    actual:\n${indentedCodeBlock(mismatch.actual)}`);
    }
  }
  if (status.wordCount && !status.wordCount.ok) {
    const max = status.wordCount.max ?? 'unbounded';
    lines.push(`  - word count: ${status.wordCount.actual} outside ${status.wordCount.min}-${max}`);
  }
  if (status.noiseRatio && !status.noiseRatio.ok) {
    lines.push(`  - noise ratio: ${status.noiseRatio.ratio}`);
  }
  return lines;
}

function renderMarkdown(report) {
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

  const qualityDetails = report.results.flatMap(qualityGateLines);
  if (qualityDetails.length > 0) {
    lines.push('', '## Quality Gate Details', '');
    lines.push(...qualityDetails);
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

function buildReport(results) {
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
  let corpus = JSON.parse(await readFile(corpusPath, 'utf8'));
  if (options.cases.length > 0) {
    const selected = new Set(options.cases);
    corpus = corpus.filter((testCase) => selected.has(testCase.id));
  }
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

  if (options.jsonOnly) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    console.log(renderMarkdown(report));
  }
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

export {
  buildReport,
  expectationStatus,
  normalizeExpectation,
  noiseRatioStatus,
  renderMarkdown,
  wordCountStatus,
};
