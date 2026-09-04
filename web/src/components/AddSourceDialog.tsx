import { useState } from 'react';
import { CheckCircle2, Sparkles, Type, Link2, FileUp, X, XCircle } from 'lucide-react';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { ingest } from '../api';
import type { SourceItem } from '../api';
import { isDesktopClient } from '../runtime';
import { chooseSourceFiles, ingestFiles } from '../local-api';
import {
  fileIngestResult,
  ingestErrorMessage,
  mergeImportedSources,
  sourceImportFailureDetail,
  sourceImportFailureHint,
  sourceImportFailureTitle,
  sourceImportSaveUrlLabel,
  sourceImportSaveUrlProgressLabel,
  sourceImportStorageLabel,
  sourceImportProgressLabel,
  sourceReadyLabel,
} from '../lib/source-ingest';
import { agentDisplayName, type AgentKind } from './terminal/terminal-contract';

interface AddSourceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  branch: string;
  workspaceName: string;
  workspaceSlug: string;
  defaultAgent: AgentKind;
  onImported: () => void;
  onOrganize?: (sources: SourceItem[]) => void;
}

type SourceTab = 'text' | 'url' | 'file';

export function AddSourceDialog({
  open,
  onOpenChange,
  branch,
  workspaceName,
  workspaceSlug,
  defaultAgent,
  onImported,
  onOrganize,
}: AddSourceDialogProps) {
  const desktop = isDesktopClient();
  const [activeTab, setActiveTab] = useState<SourceTab>('url');
  const [content, setContent] = useState('');
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [importedSources, setImportedSources] = useState<SourceItem[]>([]);
  const [importFailed, setImportFailed] = useState(false);

  const reset = () => {
    setContent('');
    setSelectedFiles([]);
    setError('');
    setImportedSources([]);
    setImportFailed(false);
  };

  const handleOpenChange = (isOpen: boolean) => {
    if (!isOpen) reset();
    onOpenChange(isOpen);
  };

  const handleChooseFiles = async () => {
    try {
      const picked = await chooseSourceFiles();
      if (picked.length === 0) return;
      setSelectedFiles((current) => [...new Set([...current, ...picked])]);
      setError('');
    } catch (err) {
      setError(ingestErrorMessage(err) || 'Failed to choose files.');
    }
  };

  const fileName = (path: string) => path.split(/[\\/]/).pop() || path;

  const handleSubmitFiles = async () => {
    setLoading(true);
    setError('');
    setImportFailed(false);
    try {
      const outcomes = await ingestFiles(workspaceSlug, selectedFiles);
      const result = fileIngestResult(outcomes);
      const imported = outcomes.flatMap((outcome) => outcome.source ? [outcome.source] : []);
      if (imported.length) {
        setImportedSources((current) => mergeImportedSources(current, imported));
        onImported();
      }
      if (result.shouldClose) {
        setSelectedFiles([]);
      } else {
        setSelectedFiles(result.remainingFiles);
        setError(result.error);
        if (!imported.length) setImportFailed(true);
      }
    } catch (err) {
      setError(ingestErrorMessage(err));
      setImportFailed(true);
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (loading) return;
    if (activeTab === 'file') {
      if (selectedFiles.length === 0) return;
      await handleSubmitFiles();
      return;
    }
    if (!content.trim()) return;
    setLoading(true);
    setError('');
    setImportFailed(false);
    try {
      const source = await ingest(activeTab, content, branch, undefined, workspaceSlug) as SourceItem;
      setImportedSources((current) => mergeImportedSources(current, [source]));
      setContent('');
      onImported();
    } catch (err) {
      setError(ingestErrorMessage(err));
      setImportFailed(true);
    } finally {
      setLoading(false);
    }
  };

  const handleSaveUrl = async () => {
    if (!content.trim()) return;
    setLoading(true);
    setError('');
    try {
      const source = await ingest('url-reference', content, branch, undefined, workspaceSlug) as SourceItem;
      setImportedSources((current) => mergeImportedSources(current, [source]));
      setImportFailed(false);
      setError('');
      setContent('');
      onImported();
    } catch (err) {
      setError(ingestErrorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  const canSubmit = activeTab === 'file' ? selectedFiles.length > 0 : !!content.trim();
  const defaultAgentName = agentDisplayName(defaultAgent);
  const showingResult = importedSources.length > 0 || importFailed;
  const importFailedCompletely = importFailed && importedSources.length === 0;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>
            Add Source to{' '}
            <span className="text-foreground/70 italic">{workspaceName}</span>
          </DialogTitle>
        </DialogHeader>

        {showingResult ? (
          <div className="space-y-5 py-2">
            <div className="flex items-start gap-3 rounded-lg border bg-muted/35 p-4">
              {importFailedCompletely ? (
                <XCircle className="mt-0.5 h-5 w-5 shrink-0 text-red" />
              ) : (
                <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-green" />
              )}
              <div className="min-w-0">
                <p className="font-medium">
                  {importFailedCompletely
                    ? sourceImportFailureTitle(activeTab)
                    : `${importedSources.length} source${importedSources.length === 1 ? '' : 's'} imported`}
                </p>
                <p className="mt-1 text-sm text-muted-foreground">
                  {importFailedCompletely
                    ? sourceImportFailureDetail(error)
                    : sourceImportStorageLabel(desktop)}
                </p>
              </div>
            </div>
            {error && !importFailedCompletely && (
              <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red">
                <p className="font-medium">Some files still need attention</p>
                <p className="mt-1 text-xs leading-relaxed">{error}</p>
              </div>
            )}
            {importFailedCompletely ? (
              <div>
                <p className="text-sm font-medium">
                  {activeTab === 'url' ? 'Nothing was saved yet' : 'Nothing was saved'}
                </p>
                <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                  {sourceImportFailureHint(activeTab)}
                </p>
              </div>
            ) : desktop && onOrganize ? (
              <div>
                <p className="text-sm font-medium">{sourceReadyLabel(defaultAgentName)}</p>
                <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                  This opens an isolated Agent Change. You can review its knowledge edits before merging them into the Current Draft.
                </p>
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              {importFailedCompletely && activeTab === 'url' && (
                <Button
                  type="button"
                  disabled={loading || !content.trim()}
                  onClick={() => { void handleSaveUrl(); }}
                >
                  {loading ? sourceImportSaveUrlProgressLabel() : sourceImportSaveUrlLabel()}
                </Button>
              )}
              {importFailedCompletely && activeTab !== 'url' && (
                <Button
                  type="button"
                  disabled={loading}
                  onClick={() => {
                    setImportFailed(false);
                    setError('');
                  }}
                >
                  Try again
                </Button>
              )}
              {error && selectedFiles.length > 0 && !importFailedCompletely && (
                <Button
                  type="button"
                  variant="outline"
                  disabled={loading}
                  onClick={() => { void handleSubmitFiles(); }}
                >
                  {loading
                    ? sourceImportProgressLabel('file', selectedFiles.length)
                    : `Retry ${selectedFiles.length} failed file${selectedFiles.length === 1 ? '' : 's'}`}
                </Button>
              )}
              {!importFailedCompletely && desktop && onOrganize && (
                <Button
                  type="button"
                  onClick={() => {
                    onOrganize(importedSources);
                    handleOpenChange(false);
                  }}
                >
                  <Sparkles className="h-4 w-4" />
                  Open {defaultAgentName} to organize
                </Button>
              )}
              <Button type="button" variant="outline" onClick={() => handleOpenChange(false)}>
                Done
              </Button>
            </div>
          </div>
        ) : (
        <form onSubmit={handleSubmit} className="space-y-4">
          <Tabs
            value={activeTab}
            onValueChange={(v) => {
              setActiveTab(v as SourceTab);
              setError('');
            }}
          >
            <TabsList>
              <TabsTrigger value="text">
                <Type className="h-4 w-4" />
                Text
              </TabsTrigger>
              <TabsTrigger value="url">
                <Link2 className="h-4 w-4" />
                URL
              </TabsTrigger>
              {desktop && (
                <TabsTrigger value="file">
                  <FileUp className="h-4 w-4" />
                  File
                </TabsTrigger>
              )}
            </TabsList>

            <TabsContent value="text" className="mt-3">
              <Textarea
                value={content}
                onChange={(e) => setContent(e.target.value)}
                placeholder="Paste or type your content here..."
                rows={8}
              />
            </TabsContent>

            <TabsContent value="url" className="mt-3 space-y-1.5">
              <Input
                type="url"
                value={content}
                onChange={(e) => setContent(e.target.value)}
                placeholder="https://example.com/article"
              />
              <p className="text-xs text-muted-foreground">
                CoWiki fetches the page and saves the article as Markdown. JavaScript-only apps cannot be extracted yet.
              </p>
            </TabsContent>

            {desktop && (
              <TabsContent value="file" className="mt-3 space-y-3">
                <Button type="button" variant="outline" onClick={() => { void handleChooseFiles(); }} className="w-full">
                  <FileUp className="h-4 w-4" />
                  Choose files…
                </Button>
                {selectedFiles.length > 0 && (
                  <ul className="max-h-40 space-y-1 overflow-y-auto rounded-md border p-1.5">
                    {selectedFiles.map((path) => (
                      <li key={path} className="flex items-center justify-between gap-2 rounded px-2 py-1 text-sm hover:bg-muted/50">
                        <span className="truncate">{fileName(path)}</span>
                        <button
                          type="button"
                          onClick={() => setSelectedFiles((current) => current.filter((p) => p !== path))}
                          className="shrink-0 text-muted-foreground hover:text-foreground"
                          aria-label={`Remove ${fileName(path)}`}
                        >
                          <X className="h-3.5 w-3.5" />
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
                <p className="text-xs text-muted-foreground">
                  PDF, Word, PowerPoint, and spreadsheet files are converted to text on import. Unsupported formats are reported per file.
                </p>
              </TabsContent>
            )}
          </Tabs>

          {error && <p className="text-sm text-red">{error}</p>}

          <Button type="submit" disabled={!canSubmit || loading}>
            {loading
              ? sourceImportProgressLabel(activeTab, selectedFiles.length)
              : activeTab === 'file' && selectedFiles.length > 1
                ? `Add ${selectedFiles.length} Sources`
                : 'Add Source'}
          </Button>
        </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
