import assert from 'node:assert/strict';
import test from 'node:test';

import { restoreSystemFrontmatter, splitSystemFrontmatter } from '../src/lib/page-frontmatter.ts';

test('system frontmatter is hidden from the editable document and restored exactly', () => {
  const original = '---\ntype: Note\ntitle: "Test"\nsummary: ""\n---\n\nEditable text\n- [ ] Task';
  const page = splitSystemFrontmatter(original);

  assert.equal(page.body, 'Editable text\n- [ ] Task');
  assert.equal(restoreSystemFrontmatter(page.systemFrontmatter, page.body), original);
});

test('plain Markdown remains fully editable', () => {
  assert.deepEqual(splitSystemFrontmatter('# Hello'), { systemFrontmatter: '', body: '# Hello' });
});

test('read-only Source rendering hides OKF frontmatter', () => {
  const source = splitSystemFrontmatter(
    '---\ntitle: "https://example.com"\ntype: Source\n---\n\nhttps://example.com',
  );

  assert.equal(source.body, 'https://example.com');
});

test('read-only Source rendering hides resource provenance fields', () => {
  const source = splitSystemFrontmatter(`---
type: Source
title: "Example"
resource: "https://example.com/article"
requested_url: "https://example.com/article"
final_url: "https://www.example.com/article"
timestamp: "2026-09-01T10:30:00Z"
content_hash: "sha256:abc"
warnings:
  - WEB_SPA_SHELL_DETECTED
---

Article body
`);

  assert.equal(source.body, 'Article body\n');
  assert.match(source.systemFrontmatter, /requested_url: "https:\/\/example.com\/article"/);
  assert.match(source.systemFrontmatter, /WEB_SPA_SHELL_DETECTED/);
});
