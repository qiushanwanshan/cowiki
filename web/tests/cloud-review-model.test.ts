import assert from 'node:assert/strict';
import test from 'node:test';

import { CloudApiError } from '../src/cloud/client.ts';
import {
  cloudDiffToFileDiffs,
  cloudMergeErrorMessage,
} from '../src/cloud/cloud-review-model.ts';

test('Cloud unified patches adapt to the shared client DiffView model', () => {
  const diffs = cloudDiffToFileDiffs({
    baseOid: 'base',
    headOid: 'head',
    files: [
      { path: 'index.md', status: 'modified', additions: 2, deletions: 1 },
      { path: 'new.md', status: 'added', additions: 1, deletions: 0 },
    ],
    patch: [
      'diff --git a/index.md b/index.md',
      'index 1111111..2222222 100644',
      '--- a/index.md',
      '+++ b/index.md',
      '@@ -1,2 +1,3 @@',
      ' # Title',
      '-Old',
      '+New',
      '+More',
      'diff --git a/new.md b/new.md',
      'new file mode 100644',
      '--- /dev/null',
      '+++ b/new.md',
      '@@ -0,0 +1 @@',
      '+Hello',
      '',
    ].join('\n'),
  });

  assert.equal(diffs.length, 2);
  assert.equal(diffs[0]?.path, 'index.md');
  assert.deepEqual(diffs[0]?.hunks[0]?.lines, [
    { kind: 'ctx', old_line: 1, new_line: 1, text: '# Title' },
    { kind: 'del', old_line: 2, new_line: null, text: 'Old' },
    { kind: 'add', old_line: null, new_line: 2, text: 'New' },
    { kind: 'add', old_line: null, new_line: 3, text: 'More' },
  ]);
  assert.equal(diffs[0]?.old_content, '');
  assert.equal(diffs[0]?.new_content, '');
  assert.equal(diffs[1]?.old_content, null);
  assert.equal(diffs[1]?.new_content, '');
});

test('Cloud merge failures distinguish stale heads from real file conflicts', () => {
  assert.equal(
    cloudMergeErrorMessage(new CloudApiError(409, 'server detail', 'stale_head')),
    'This pull request changed. Review the latest head before merging.',
  );
  assert.equal(
    cloudMergeErrorMessage(new CloudApiError(
      409,
      'server detail',
      'merge_conflict',
      ['wiki/index.md', 'wiki/roadmap.md'],
    )),
    'This pull request conflicts with the latest Cloud main in: wiki/index.md, wiki/roadmap.md.',
  );
  assert.equal(
    cloudMergeErrorMessage(new CloudApiError(409, 'Pull request is closed')),
    'Pull request is closed',
  );
});
