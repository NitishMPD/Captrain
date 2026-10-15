import { useCallback, useEffect, useRef, useState } from 'react';
import { ConfirmDialog, NameDialog } from '../Dialogs';
import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import { CaptureList, linkTitle, linkOpenLabel, type Layout } from '../library/CaptureList';
import { native, type CaptureItem, type Folder, type Settings } from '../native';
import { formatShortcut } from '../settings/shortcut';
import { errorMessage } from '../util';
import { CaptureControl } from './CaptureControl';
import { FolderTabs } from './FolderTabs';
import { fly } from './flight';

export type Tab = 'all' | 'captures' | 'exports';
export interface Filters { tab: Tab; folderId: string | null; query: string }
export const NO_FILTERS: Filters = { tab: 'all', folderId: null, query: '' };

const TABS: { id: Tab; label: string }[] = [{ id: 'all', label: 'All' }, { id: 'captures', label: 'Captures' }, { id: 'exports', label: 'Exports' }];

type Dialog =
  | { name: 'new-folder' }
  | { name: 'rename'; folder: Folder }
  | { name: 'rename-link'; item: CaptureItem }
  | { name: 'delete-folder'; folder: Folder }
  | { name: 'delete'; item: CaptureItem };

interface Props {
  items: CaptureItem[];
  total: number;
  thumbs: Record<string, string>;
  loaded: boolean;
  error: string | null;
  freshId: string | null;
  rev: number;
  settings: Settings | null;
  filters: Filters;
  onFilters: (filters: Filters) => void;
  onMore: () => void;
  onOpen: (item: CaptureItem) => void;
  onSettings: () => void;
  onRecord: () => void;
  recordingActive: boolean;
  onPatch: (patch: Partial<Settings>) => void;
  onSettingsSaved: (settings: Settings) => void;
  /** Items or folders changed in a way the list must reload. */
  onChanged: () => void;
  onError: (message: string) => void;
}

function useFolders(rev: number, onError: (m: string) => void) {
  const [folders, setFolders] = useState<Folder[]>([]);
  useEffect(() => {
    let live = true;
    native.folders().then(f => { if (live) setFolders(f); }).catch(e => { if (live) onError(errorMessage(e)); });
    return () => { live = false; };
  }, [rev]); // eslint-disable-line react-hooks/exhaustive-deps
  return folders;
}

export function Shelf(p: Props) {
  const { items, filters, settings, freshId, thumbs, onError, onChanged } = p;
  const [layout, setLayout] = useState<Layout>(() => (localStorage.getItem('layout') === 'grid' ? 'grid' : 'list'));
  const [searching, setSearching] = useState(filters.query !== '');
  const [input, setInput] = useState(filters.query);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState('');
  const folders = useFolders(p.rev, onError);
  const newMenu = useContextMenu();
  const panel = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLElement>(null);
  const played = useRef<string | null>(null);
  const flight = useRef<(() => void) | null>(null);

  useEffect(() => { setInput(filters.query); }, [filters.query]);
  useEffect(() => {
    const t = setTimeout(() => { if (input.trim() !== filters.query) p.onFilters({ ...filters, query: input.trim() }); }, 200);
    return () => clearTimeout(t);
  }, [input]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(''), 2200);
    return () => clearTimeout(t);
  }, [toast]);

  // The thumbnail is decoded before it lands in `thumbs`, so the flight never shows a blank frame.
  useEffect(() => {
    if (!freshId || played.current === freshId || !thumbs[freshId] || items[0]?.id !== freshId) return;
    const target = body.current?.querySelector<HTMLElement>(`[data-thumb="${CSS.escape(freshId)}"]`);
    const from = panel.current?.getBoundingClientRect();
    if (!target || !from) return;
    played.current = freshId;
    body.current!.scrollTop = 0;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    flight.current?.();
    flight.current = fly(thumbs[freshId], from, target);
  }, [freshId, thumbs, items]);
  useEffect(() => () => flight.current?.(), []);

  const setLayoutPersist = (next: Layout) => { setLayout(next); localStorage.setItem('layout', next); };
  const report = useCallback((e: unknown) => onError(errorMessage(e)), [onError]);

  const run = async (task: () => Promise<void>) => {
    setBusy(true);
    try { await task(); } catch (e) { report(e); } finally { setBusy(false); }
  };

  const move = (item: CaptureItem, folderId: string | null) =>
    native.moveCapture(item.id, folderId).then(onChanged).catch(report);

  const itemMenu = (item: CaptureItem): MenuItem[] => [
    { label: item.kind === 'url' ? linkOpenLabel(item) : item.kind === 'video' ? 'Play video' : 'Open', onSelect: () => p.onOpen(item) },
    ...(item.kind === 'video' ? [] : [{ label: 'Copy', onSelect: () => native.copy(item.id).then(() => setToast('Copied to clipboard')).catch(report) }]),
    { label: 'Show in Explorer', onSelect: () => native.reveal(item.id).catch(report) },
    ...(item.kind === 'url' ? [{ label: 'Rename...', onSelect: () => setDialog({ name: 'rename-link', item }) }] : []),
    { label: 'Save as…', onSelect: () => native.save(item.id).then(ok => ok && setToast('Saved')).catch(report) },
    { label: 'Move to folder', submenu: [
      { label: 'No folder', checked: item.folderId === null, onSelect: () => move(item, null) },
      ...folders.map(f => ({ label: f.name, checked: item.folderId === f.id, onSelect: () => move(item, f.id) })),
    ] },
    { label: 'Delete…', danger: true, separatorBefore: true, onSelect: () => setDialog({ name: 'delete', item }) },
  ];

  const folderMenu = (folder: Folder): MenuItem[] => [
    { label: filters.folderId === folder.id ? 'Show all captures' : 'Show this folder', onSelect: () => p.onFilters({ ...filters, folderId: filters.folderId === folder.id ? null : folder.id }) },
    { label: settings?.activeFolder === folder.id ? 'Stop using as default' : 'Use as default folder', onSelect: () => p.onPatch({ activeFolder: settings?.activeFolder === folder.id ? null : folder.id }) },
    ...(folder.path ? [{ label: 'Open in Explorer', onSelect: () => native.openFolder(folder.id).catch(report) }] : []),
    { label: 'Rename…', separatorBefore: true, onSelect: () => setDialog({ name: 'rename', folder }) },
    { label: 'Delete folder…', danger: true, onSelect: () => setDialog({ name: 'delete-folder', folder }) },
  ];

  const addPcFolder = () => native.createPermanentFolder().then(f => f && onChanged()).catch(report);

  const filtered = filters.query !== '' || filters.folderId !== null || filters.tab !== 'all';
  const closeDialog = () => setDialog(null);

  return (
    <>
      <div className="view" inert={dialog !== null} ref={panel}>
        <div className="sheet">
          <header className="header">
            <button className="icon-btn brand-home" onClick={() => p.onFilters(NO_FILTERS)} aria-label="Captrain — show everything" title="Captrain — show everything"><img src="/captrain.svg" width="28" height="28" alt="" /></button>
            <div className="seg tabs" role="group" aria-label="Show">
              {TABS.map(t => (
                <button key={t.id} aria-pressed={filters.tab === t.id} onClick={() => p.onFilters({ ...filters, tab: t.id, folderId: t.id === 'all' ? filters.folderId : null })}>{t.label}</button>
              ))}
            </div>
            <button className="icon-btn" onClick={p.onSettings} aria-label="Settings"><Icon name="settings" /></button>
          </header>
          <div className="toolbar">
            <div className="seg layout" role="group" aria-label="Layout">
              <button aria-pressed={layout === 'list'} aria-label="List view" onClick={() => setLayoutPersist('list')}><Icon name="list" /></button>
              <button aria-pressed={layout === 'grid'} aria-label="Grid view" onClick={() => setLayoutPersist('grid')}><Icon name="library" /></button>
            </div>
            {searching
              ? <input className="search" type="search" autoFocus value={input} onChange={e => setInput(e.target.value)}
                  onKeyDown={e => { if (e.key !== 'Escape') return; e.preventDefault(); if (input === '') setSearching(false); else setInput(''); }}
                  placeholder="Search URL, name, kind or date" aria-label="Search library" />
              : <span className="spacer" />}
            <button className="icon-btn" aria-label="New folder" title="New folder" aria-haspopup="menu" aria-expanded={newMenu.isOpen}
              onClick={e => newMenu.openBelow([
                { label: 'Captrain folder…', onSelect: () => setDialog({ name: 'new-folder' }) },
                { label: 'Folder on this PC…', onSelect: addPcFolder },
              ], e.currentTarget)}><Icon name="folderPlus" /></button>
            {newMenu.menu}
            <button className="icon-btn" aria-pressed={searching} aria-label={searching ? 'Close search' : 'Search'}
              onClick={() => { if (searching) { setInput(''); } setSearching(!searching); }}><Icon name="search" /></button>
          </div>
          {filters.tab === 'exports' && (
            <FolderTabs folders={folders.filter(f => f.path)} selected={filters.folderId} active={settings?.activeFolder ?? null} layout={layout} emptyLabel="Add a folder from this PC to save captures into"
              onSelect={id => p.onFilters({ ...filters, folderId: id })} onCreate={addPcFolder} menuFor={folderMenu}
              onDropItem={(id, folderId) => { const item = items.find(i => i.id === id); if (item && item.folderId !== folderId) move(item, folderId); }} />
          )}
          {filters.tab === 'all' && (
            <FolderTabs folders={folders} selected={filters.folderId} active={settings?.activeFolder ?? null}
              onSelect={id => p.onFilters({ ...filters, folderId: id })} onCreate={() => setDialog({ name: 'new-folder' })} menuFor={folderMenu}
              onDropItem={(id, folderId) => { const item = items.find(i => i.id === id); if (item && item.folderId !== folderId) move(item, folderId); }} />
          )}
          <main className="body" ref={body}>
            {p.error ? <p className="empty">{p.error}</p>
              : !p.loaded ? null
              : items.length === 0 ? (
                <p className="empty">
                  {filtered ? 'Nothing matches these filters.' : 'No captures yet.'}
                  {!filtered && settings && <> Press <kbd>{formatShortcut(settings.shortcuts.region)}</kbd> or use Capture below.</>}
                </p>
              ) : <CaptureList items={items} thumbs={thumbs} layout={layout} freshId={freshId} onOpen={p.onOpen} menuFor={itemMenu} />}
            {items.length < p.total && <button className="btn more" onClick={p.onMore}>Show more ({p.total - items.length})</button>}
          </main>
          <p className="toast" role="status">{toast}</p>
        </div>
        <CaptureControl recordingActive={p.recordingActive} settings={settings} folders={folders} onPatch={p.onPatch} onSettings={p.onSettingsSaved}
          onRecord={p.onRecord} onNewFolder={() => setDialog({ name: 'new-folder' })} onPcFolder={addPcFolder} onError={onError} />
      </div>
      {dialog?.name === 'new-folder' && (
        <NameDialog title="New folder" submit="Create" onCancel={closeDialog}
          onSubmit={async name => { await native.createFolder(name); onChanged(); closeDialog(); }} />
      )}
      {dialog?.name === 'rename' && (
        <NameDialog title="Rename folder" initial={dialog.folder.name} submit="Rename" onCancel={closeDialog}
          onSubmit={async name => { await native.renameFolder(dialog.folder.id, name); onChanged(); closeDialog(); }} />
      )}
      {dialog?.name === 'rename-link' && (
        <NameDialog title="Rename link" label="Link name" maxLength={120} initial={linkTitle(dialog.item)} submit="Rename" onCancel={closeDialog}
          onSubmit={async name => { await native.renameLink(dialog.item.id, name); onChanged(); closeDialog(); }} />
      )}
      {dialog?.name === 'delete-folder' && (
        <ConfirmDialog title={`Delete “${dialog.folder.name}”?`} confirm="Delete folder" busy={busy} onCancel={closeDialog}
          body={dialog.folder.path ? 'The folder is removed from Captrain. Its captures stay in your library, and files already saved on your PC are kept.' : 'The folder is removed. Its captures stay in your library without a folder.'}
          onConfirm={() => run(async () => {
            await native.deleteFolder(dialog.folder.id);
            if (filters.folderId === dialog.folder.id) p.onFilters({ ...filters, folderId: null });
            if (settings?.activeFolder === dialog.folder.id) p.onPatch({ activeFolder: null });
            onChanged();
            closeDialog();
          })} />
      )}
      {dialog?.name === 'delete' && (
        <ConfirmDialog title="Delete this capture?" confirm="Delete" busy={busy} onCancel={closeDialog}
          body="This item will be permanently removed from Captrain. Exported copies are kept."
          onConfirm={() => run(async () => { await native.delete(dialog.item.id); onChanged(); closeDialog(); })} />
      )}
    </>
  );
}
