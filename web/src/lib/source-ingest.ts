import type { IngestFileOutcome, SourceItem } from '../api.ts';

export interface FileIngestResult {
  remainingFiles: string[];
  error: string;
  shouldClose: boolean;
}

export type SourceImportKind = 'text' | 'url' | 'file';

export function sourceImportProgressLabel(kind: SourceImportKind, fileCount: number): string {
  if (kind === 'file') {
    return `Reading and extracting ${fileCount} source${fileCount === 1 ? '' : 's'}…`;
  }
  if (kind === 'url') {
    return 'Fetching and extracting page…';
  }
  return 'Saving and indexing source…';
}

export function sourceReadyLabel(agentName: string): string {
  return `Ready for ${agentName} to organize`;
}

export function sourceImportStorageLabel(desktop: boolean): string {
  return desktop
    ? 'Saved locally and added to the OKF and search indexes.'
    : 'Added to this Space and its search index.';
}

export function sourceImportFailureTitle(kind: SourceImportKind): string {
  if (kind === 'url') return "Couldn't import this page";
  if (kind === 'file') return "Couldn't import these files";
  return "Couldn't import this source";
}

export function sourceImportFailureHint(kind: SourceImportKind): string {
  if (kind === 'url') {
    return 'Save the link as a Source, or switch to Text and paste the article.';
  }
  if (kind === 'file') {
    return 'You can retry the failed files, or choose different ones.';
  }
  return 'You can edit the text and try again.';
}

export function sourceImportSaveUrlLabel(): string {
  return 'Save URL';
}

export function sourceImportSaveUrlProgressLabel(): string {
  return 'Saving URL…';
}

export function ingestErrorMessage(cause: unknown): string {
  if (typeof cause === 'string' && cause.trim()) return cause.trim();
  if (cause instanceof Error && cause.message.trim()) return cause.message.trim();
  if (cause && typeof cause === 'object') {
    const record = cause as Record<string, unknown>;
    for (const key of ['message', 'error']) {
      const value = record[key];
      if (typeof value === 'string' && value.trim()) return value.trim();
    }
  }
  return '';
}

export function sourceImportFailureDetail(error: string): string {
  const trimmed = error.trim();
  if (!trimmed) return 'Something went wrong while importing.';
  return trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
}

export function mergeImportedSources(
  current: SourceItem[],
  imported: SourceItem[],
): SourceItem[] {
  const sources = new Map(current.map((source) => [source.filename, source]));
  imported.forEach((source) => sources.set(source.filename, source));
  return [...sources.values()];
}

export function sourceOrganizationTask(sources: SourceItem[]): string {
  const paths = sources
    .map((source) => source.filename.startsWith('sources/')
      ? source.filename
      : `sources/${source.filename}`)
    .map((path) => `- ${path}`)
    .join('\n');
  return [
    'Organize the newly imported OKF Source files below into durable knowledge.',
    'Read each Source, update the appropriate Concepts and indexes, preserve provenance and Source frontmatter, and do not modify the Source files.',
    paths,
  ].join('\n');
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

export function fileIngestResult(outcomes: IngestFileOutcome[]): FileIngestResult {
  const failed = outcomes.filter((outcome) => outcome.error);
  if (failed.length === 0) {
    return { remainingFiles: [], error: '', shouldClose: true };
  }

  return {
    remainingFiles: failed.map((outcome) => outcome.sourcePath),
    error: `${failed.length} of ${outcomes.length} file(s) failed: ${failed
      .map((outcome) => `${fileName(outcome.sourcePath)} (${outcome.error})`)
      .join(', ')}`,
    shouldClose: false,
  };
}
