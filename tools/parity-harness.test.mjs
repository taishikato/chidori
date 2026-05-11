import assert from 'node:assert/strict';
import test from 'node:test';

import { buildReport, renderMarkdown } from './parity-harness.mjs';

test('buildReport does not include a dynamic generation timestamp', () => {
  const report = buildReport([]);

  assert.equal(Object.hasOwn(report, 'generatedAt'), false);
  assert.equal(renderMarkdown(report).includes('Generated:'), false);
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
