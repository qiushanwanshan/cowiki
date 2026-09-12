import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createServer } from 'vite';
import { workspaceContextStatus } from '../src/lib/workspace-context.ts';

test('the visible header distinguishes local-only, pending upload and Cloud updates', async () => {
  const vite = await createServer({ root: fileURLToPath(new URL('../', import.meta.url)), appType: 'custom', logLevel: 'silent', server: { middlewareMode: true } });
  try {
    const { WorkspaceContextBadge } = await vite.ssrLoadModule('/src/components/layout/WorkspaceContextBadge.tsx');
    for (const [state, label] of [
      ['unlinked', 'Local only'], ['dirty', 'Not uploaded'], ['needsSync', 'Cloud update'], ['conflicted', 'Attention'],
    ] as const) {
      const html = renderToStaticMarkup(React.createElement(WorkspaceContextBadge, {
        context: workspaceContextStatus({ desktop: true, state }), connected: state !== 'unlinked',
      }));
      // Inspect visible text, not the tooltip: readers must not need to hover to learn where their changes live.
      const visible = html.replace(/<[^>]*>/g, '');
      assert.ok(visible.includes(label), `Expected ${label} in visible header, got ${visible}`);
    }
  } finally {
    await vite.close();
  }
});
