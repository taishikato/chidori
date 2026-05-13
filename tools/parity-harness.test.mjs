import assert from 'node:assert/strict';
import test from 'node:test';

import { buildReport, expectationStatus, noiseRatioStatus, parseArgs, renderMarkdown } from './parity-harness.mjs';

test('buildReport does not include a dynamic generation timestamp', () => {
  const report = buildReport([]);

  assert.equal(Object.hasOwn(report, 'generatedAt'), false);
  assert.equal(renderMarkdown(report).includes('Generated:'), false);
});

test('parseArgs ignores a missing --case value before another flag', () => {
  const options = parseArgs(['--case', '--json']);

  assert.deepEqual(options.cases, []);
  assert.equal(options.jsonOnly, true);
});

test('renderMarkdown documents metadata gaps separately from open failures', () => {
  const report = buildReport([
    {
      id: 'real-site',
      category: 'main-content-cleanup',
      capability: 'article extraction',
      mappedStatus: 'implemented',
      status: 'parity-or-better',
      similarity: 1,
      wordCount: { chidori: 10, reference: 10 },
      metadata: {
        chidori: { author: '', published: '' },
        reference: {
          author: 'Ada Lovelace',
          published: '2026-05-10T09:30:00Z',
        },
      },
    },
  ]);

  const markdown = renderMarkdown(report);

  assert.match(markdown, /No unexplained chidori-worse-than-reference cases remain/);
  assert.match(markdown, /## Known Limitations/);
  assert.match(markdown, /real-site: missing metadata fields \(author, published\)/);
});

test('expectationStatus checks snippets, metadata, word bands, and noise ratio', () => {
  const markdown = [
    '# Parser Garden',
    '',
    'The article body is stable and useful.',
    'The article body has enough content to measure.',
    'Share this article',
  ].join('\n');
  const metadata = {
    title: 'Parser Garden',
    author: 'Ada Lovelace',
    published: '2026-05-12',
  };
  const expected = {
    contains: ['article body is stable'],
    excludes: ['newsletter popup'],
    metadata: { title: 'Parser Garden', author: 'Ada Lovelace' },
    wordCount: { min: 10, max: 20 },
    noise: ['Share this article', 'Sign up'],
    maxNoiseRatio: 0.25,
  };

  const status = expectationStatus(markdown, metadata, expected);

  assert.equal(status.ok, true);
  assert.deepEqual(status.missingExpected, []);
  assert.deepEqual(status.presentRejected, []);
  assert.deepEqual(status.metadataMismatches, []);
  assert.equal(status.wordCount.ok, true);
  assert.equal(status.noiseRatio.ok, true);
});

test('expectationStatus reports every failed quality gate', () => {
  const status = expectationStatus(
    'Short text with Subscribe now Subscribe now',
    { title: 'Wrong title', author: '' },
    {
      contains: ['missing body'],
      excludes: ['Subscribe now'],
      metadata: { title: 'Right title', author: 'Ada Lovelace' },
      wordCount: { min: 20, max: 40 },
      noise: ['Subscribe now'],
      maxNoiseRatio: 0.1,
    },
  );

  assert.equal(status.ok, false);
  assert.deepEqual(status.missingExpected, ['missing body']);
  assert.deepEqual(status.presentRejected, ['Subscribe now']);
  assert.deepEqual(status.metadataMismatches, [
    { field: 'title', expected: 'Right title', actual: 'Wrong title' },
    { field: 'author', expected: 'Ada Lovelace', actual: '' },
  ]);
  assert.equal(status.wordCount.ok, false);
  assert.equal(status.noiseRatio.ok, false);
});

test('expectationStatus treats a second argument expectation object as expectations', () => {
  const status = expectationStatus('Useful parser body', {
    contains: ['missing parser body'],
  });

  assert.equal(status.ok, false);
  assert.deepEqual(status.missingExpected, ['missing parser body']);
});

test('noiseRatioStatus counts repeated noise occurrences', () => {
  const status = noiseRatioStatus(
    'Subscribe now Subscribe now Subscribe now clean body',
    ['Subscribe now'],
    0.5,
  );

  assert.equal(status.ok, false);
  assert.deepEqual(status.matched, ['Subscribe now']);
});

test('expectationStatus preserves legacy array calls', () => {
  const status = expectationStatus('must include clean content and reject this', ['must include'], ['reject this']);

  assert.equal(status.ok, false);
  assert.deepEqual(status.missingExpected, []);
  assert.deepEqual(status.presentRejected, ['reject this']);
});

test('renderMarkdown includes quality gate failure details', () => {
  const report = buildReport([
    {
      id: 'lazy-image',
      category: 'dom-standardization',
      capability: 'lazy image normalization',
      mappedStatus: 'planned',
      status: 'chidori-worse',
      similarity: 0.4,
      wordCount: { chidori: 8, reference: 20 },
      expectations: {
        chidori: {
          ok: false,
          missingExpected: ['![Diagram](https://example.com/full.png)'],
          presentRejected: ['data:image/svg+xml'],
          metadataMismatches: [],
          wordCount: { ok: false, actual: 8, min: 10, max: 50 },
          noiseRatio: { ok: false, ratio: 0.4, matched: ['data:image/svg+xml'] },
        },
        reference: {
          ok: true,
          missingExpected: [],
          presentRejected: [],
          metadataMismatches: [],
          wordCount: { ok: true, actual: 20, min: 10, max: 50 },
          noiseRatio: { ok: true, ratio: 0, matched: [] },
        },
      },
      metadata: { chidori: {}, reference: {} },
    },
  ]);

  const markdown = renderMarkdown(report);

  assert.match(markdown, /## Quality Gate Details/);
  assert.match(markdown, /lazy-image/);
  assert.match(markdown, /missing: `!\[Diagram\]\(https:\/\/example\.com\/full\.png\)`/);
  assert.match(markdown, /rejected present: `data:image\/svg\+xml`/);
  assert.match(markdown, /word count: 8 outside 10-50/);
  assert.match(markdown, /noise ratio: 0.4/);
});

test('renderMarkdown keeps complex quality gate snippets readable', () => {
  const missing = 'missing `image`\nwith ```fence```';
  const rejected = '<img alt="bad"\nsrc="data:image/gif;base64,abc`123">';
  const report = buildReport([
    {
      id: 'complex-snippet',
      category: 'dom-standardization',
      capability: 'complex snippet rendering',
      mappedStatus: 'planned',
      status: 'chidori-worse',
      similarity: 0.2,
      wordCount: { chidori: 4, reference: 12 },
      expectations: {
        chidori: {
          ok: false,
          missingExpected: [missing],
          presentRejected: [rejected],
          metadataMismatches: [
            {
              field: 'title',
              expected: 'Expected `Title`\nline two',
              actual: 'Actual ```Title```',
            },
          ],
          wordCount: { ok: true, actual: 12, min: 5, max: 50 },
          noiseRatio: { ok: true, ratio: 0, matched: [] },
        },
        reference: {
          ok: true,
          missingExpected: [],
          presentRejected: [],
          metadataMismatches: [],
          wordCount: { ok: true, actual: 12, min: 5, max: 50 },
          noiseRatio: { ok: true, ratio: 0, matched: [] },
        },
      },
      metadata: { chidori: {}, reference: {} },
    },
  ]);

  const markdown = renderMarkdown(report);

  assert.match(markdown, /missing:\n    ````\n    missing `image`\n    with ```fence```\n    ````/);
  assert.match(markdown, /rejected present:\n    ```\n    <img alt="bad"\n    src="data:image\/gif;base64,abc`123">\n    ```/);
  assert.match(markdown, /metadata title:\n    expected:\n    ```\n    Expected `Title`\n    line two\n    ```/);
  assert.match(markdown, /actual:\n    ````\n    Actual ```Title```\n    ````/);
});
