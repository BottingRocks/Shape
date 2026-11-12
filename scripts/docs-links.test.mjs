import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  checkFile,
  collectAnchors,
  extractLinks,
  runCheck,
  slugify,
} from './docs-links.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'shape-docs-links-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}

test('slugify follows GitHub heading conventions', () => {
  assert.equal(slugify('Build and choose an output directory'), 'build-and-choose-an-output-directory');
  assert.equal(slugify('`try` / `catch` / `finally`'), 'try--catch--finally');
  assert.equal(
    slugify('Complete dispatch: preparation (`prepare.rs` + `proof.rs`)'),
    'complete-dispatch-preparation-preparers--proofrs',
  );
  assert.equal(slugify('status: exact/partial'), 'status-exactpartial');
});

test('extractLinks skips code fences and inline code but keeps code labels', () => {
  const markdown = [
    '# Title',
    '```',
    '[ignored](missing-fenced.md)',
    '```',
    '`args[1](error)`',
    '[`kept`](target.md#section)',
    'a `span',
    'across [lines](multi.md)` tail',
  ].join('\n');

  const links = extractLinks(markdown);
  assert.equal(links.length, 1);
  assert.equal(links[0].label, '`kept`');
  assert.equal(links[0].target, 'target.md#section');
  assert.equal(links[0].line, 6);
});

test('collectAnchors handles duplicates and explicit HTML anchors', () => {
  const anchors = collectAnchors(
    [
      '## Hello World',
      '## Hello World',
      '### `try` / `catch` / `finally`',
      '<a name="explicit"></a>',
      '## Foo',
      '## Foo-1',
      '## Foo',
      '```',
      '## fenced heading',
      '```',
    ].join('\n'),
  );

  assert.ok(anchors.has('hello-world'));
  assert.ok(anchors.has('hello-world-1'));
  assert.ok(anchors.has('try--catch--finally'));
  assert.ok(anchors.has('explicit'));
  assert.ok(anchors.has('foo'));
  assert.ok(anchors.has('foo-1'));
  assert.ok(anchors.has('foo-2'));
  assert.ok(!anchors.has('fenced-heading'));
});

test('unmatched backticks are literal but matched multiline spans hide links', () => {
  const links = extractLinks(
    [
      '`multi',
      '[hidden](hidden.md)`',
      '[after](after.md)',
      '`unmatched [visible](visible.md)',
      '[also](also.md)',
    ].join('\n'),
  );

  assert.deepEqual(
    links.map((link) => link.target),
    ['after.md', 'visible.md', 'also.md'],
  );
});

test('checkFile reports missing files and missing anchors', (t) => {
  const root = fixture(t);
  fs.mkdirSync(path.join(root, 'docs'), { recursive: true });
  fs.writeFileSync(path.join(root, 'docs', 'target.md'), '# Target\n\n## Section\n');
  fs.writeFileSync(
    path.join(root, 'docs', 'page.md'),
    [
      '[ok](target.md#section)',
      '[missing file](absent.md)',
      '[missing anchor](target.md#absent)',
      '```',
      '[ignored](absent.md)',
      '```',
      '`args[1](error)`',
    ].join('\n'),
  );

  const errors = checkFile(root, 'docs/page.md');
  assert.equal(errors.length, 2);
  assert.match(errors[0].message, /missing link target/);
  assert.match(errors[1].message, /missing anchor/);
});

test('tracked documentation has no broken relative links or anchors', () => {
  const { errors } = runCheck(repoRoot);
  assert.deepEqual(errors, []);
});
