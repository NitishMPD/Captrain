import { useCallback, useEffect, useRef, useState } from 'react';
import { useCaptures } from './library/useCaptures';
import { native, type CaptureItem, type Settings as SettingsData, type ShelfState } from './native';
import { Settings } from './settings/Settings';
import { NO_FILTERS, Shelf, type Filters } from './shelf/Shelf';
import { errorMessage, quiet } from './util';
import { RecordingSetup, RecordingStatus, useRecording } from './recording/Recording';
import { startUpdateChecks, useUpdates } from './updates/updates';

type View = 'shelf' | 'settings' | 'recording';

const PAGE = 20;

/** The first frame commits the thumbnail; the second follows its first paint. */
function afterPaint(run: () => void) {
  requestAnimationFrame(() => requestAnimationFrame(run));
}

export function App() {
  const recording = useRecording();
  const recordingActive = recording.status !== 'idle';
  const update = useUpdates();
  useEffect(startUpdateChecks, []);
  const [shelf, setShelf] = useState<ShelfState>({ mode: 'collapsed', hovered: false, error: null, reveal: 0 });
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [view, setView] = useState<View>('shelf');
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [limit, setLimit] = useState(PAGE);
  const [rev, setRev] = useState(0);
  const [freshId, setFreshId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const mode = useRef(shelf.mode);
  const reveal = useRef(shelf.reveal);
  const awaitingItem = useRef(false);
  const dragging = useRef(false);

  const armTimer = useCallback(() => {
    const generation = reveal.current;
    afterPaint(() => { if (mode.current === 'temporary') quiet(native.shelf('ready', generation)); });
  }, []);
  const onSettled = useCallback(() => {
    if (!awaitingItem.current) return;
    awaitingItem.current = false;
    armTimer();
  }, [armTimer]);
  const list = useCaptures(filters.query, 0, limit, rev, onSettled, filters.folderId, filters.tab === 'exports' && filters.folderId === null, false, filters.tab === 'captures');

  const changeFilters = (next: Filters) => { setFilters(next); setLimit(PAGE); };
  const bump = useCallback(() => setRev(r => r + 1), []);

  useEffect(() => {
    let disposed = false;
    const unlisten: (() => void)[] = [];
    (async () => {
      const [offState, offCapture, offLibrary, offRecording, offSettings] = await Promise.all([
        native.onOpenRecording(() => setView('recording')),
        native.onState(s => {
          const previous = mode.current;
          mode.current = s.mode;
          reveal.current = s.reveal;
          setShelf(s);
          if (s.mode === 'temporary' && previous !== 'temporary' && !awaitingItem.current) armTimer();
        }),
        native.onCapture(item => {
          awaitingItem.current = true;
          // A fresh capture must land in the first row, so any filter that could hide it is cleared.
          setFilters(NO_FILTERS);
          setLimit(PAGE);
          setFreshId(item.id);
          setRev(r => r + 1);
        }),
        // Viewer edits and folder changes; deleting the active folder also resets native's destination.
        native.onLibraryChanged(() => {
          setRev(r => r + 1);
          native.settings().then(s => { if (!disposed) setSettings(s); }).catch(() => {});
        }),
        native.onSettingsChanged(setSettings),
      ]);
      if (disposed) { offState(); offCapture(); offLibrary(); offRecording(); offSettings(); return; }
      unlisten.push(offState, offCapture, offLibrary, offRecording, offSettings);
      const [state, loaded] = await Promise.all([native.state(), native.settings()]);
      if (disposed) return;
      mode.current = state.mode;
      reveal.current = state.reveal;
      setShelf(state);
      setSettings(loaded);
      await native.shelf('ready'); // listeners are subscribed and initial state is loaded
    })().catch(e => { if (!disposed) setNotice(errorMessage(e)); });
    return () => { disposed = true; unlisten.forEach(fn => fn()); };
  }, [armTimer]);

  useEffect(() => {
    if (settings?.shortcutsOnly && shelf.mode === 'temporary') armTimer();
  }, [settings?.shortcutsOnly, shelf.mode, shelf.reveal, armTimer]);

  // A collapsed shelf always reopens on the newest captures with no filters.
  useEffect(() => {
    if (shelf.mode !== 'collapsed') return;
    setView('shelf');
    setFilters(NO_FILTERS);
    setLimit(PAGE);
    setFreshId(null);
  }, [shelf.mode]);

  // The window is shown without activation; give the page keyboard focus so Escape reaches it.
  useEffect(() => { if (shelf.mode === 'manual') window.focus(); }, [shelf.mode]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      if (view === 'settings') setView('shelf'); else quiet(native.shelf('collapse'));
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [view]);

  const patch = useCallback((change: Partial<SettingsData>) => {
    if (!settings) return;
    native.updateSettings({ ...settings, ...change }).then(setSettings).catch(e => setNotice(errorMessage(e)));
  }, [settings]);

  const error = notice ?? shelf.error;
  const dismiss = () => { setNotice(null); if (shelf.error) quiet(native.shelf('clearError')); };
  const openViewer = (item: CaptureItem) =>
    (item.kind === 'url' ? native.openLink(item.id) : item.kind === 'video' ? native.playRecording(item.id) : native.openViewer(item.id, filters.query, filters.folderId, filters.tab === 'exports' && filters.folderId === null)).catch(e => setNotice(errorMessage(e)));

  if (shelf.mode === 'capturing') return null;

  const edge = settings?.edge ?? 'right';
  const root = {
    className: `root ${edge}`,
    'data-mode': shelf.mode,
    onPointerEnter: () => quiet(native.shelf('enter')),
    onPointerLeave: () => quiet(native.shelf('leave')),
  };

  if (settings?.shortcutsOnly && shelf.mode === 'collapsed') return null;

  if (settings?.shortcutsOnly && shelf.mode === 'temporary') {
    return (
      <div {...root}>
        <section className="panel capture-notification" aria-label="Capture notification">
          <div className="capture-notification-message" role={error ? 'alert' : 'status'}>
            <strong>{error ? 'Captrain needs attention' : 'Capture saved'}</strong>
            <span>{error ?? 'Saved to your library.'}</span>
          </div>
          <div className="capture-notification-actions">
            <button className="btn" onClick={() => quiet(native.shelf('open'))}>Open shelf</button>
            <button className="btn" onClick={() => { dismiss(); quiet(native.shelf('collapse')); }}>Dismiss</button>
          </div>
        </section>
      </div>
    );
  }

  if (shelf.mode === 'collapsed') {
    return (
      <div {...root}>
        <button className="bar" data-hovered={shelf.hovered} data-recording={recordingActive} aria-label={recordingActive ? "Open recording controls" : "Open Captrain shelf"} title="Click to open; drag to move along either screen edge. Arrow keys adjust height."
          onPointerDown={async e => {
            if (e.button !== 0 || dragging.current) return;
            e.preventDefault();
            if (recordingActive) { quiet(native.shelf('open')); return; }
            dragging.current = true;
            try { if (!await native.dragShelf()) await native.shelf('open'); }
            catch (error) { setNotice(errorMessage(error)); quiet(native.shelf('open')); }
            finally { dragging.current = false; }
          }}
          onKeyDown={e => {
            if (!settings || !['ArrowUp', 'ArrowDown'].includes(e.key)) return;
            e.preventDefault();
            patch({ edgePosition: Math.max(0, Math.min(1, settings.edgePosition + (e.key === 'ArrowUp' ? -0.05 : 0.05))) });
          }}
          onClick={e => { if (e.detail === 0) quiet(native.shelf('open')); }}>
          <span className="bar-line" />
        </button>
      </div>
    );
  }

  return (
    <div {...root} onPointerDownCapture={() => { if (shelf.mode === 'temporary') quiet(native.shelf('interact')); }}>
      <section className="panel" aria-label="Captrain shelf">
        {recordingActive && <RecordingStatus state={recording} onError={setNotice} />}
        {view === 'shelf' && ['available', 'downloaded'].includes(update.status) && (
          <div className="banner" role="status">
            <span>Captrain {update.available?.version} {update.status === 'downloaded' ? 'is ready to install' : 'is available'}.</span>
            <button className="btn" onClick={() => setView('settings')}>View update</button>
          </div>
        )}
        {error && (
          <div className="banner" role="alert">
            <span>{error}</span>
            <button className="icon-btn" onClick={dismiss} aria-label="Dismiss message">×</button>
          </div>
        )}
        {view === 'shelf' && (
          <Shelf recordingActive={recordingActive} items={list.items} total={list.total} thumbs={list.thumbs} loaded={list.loaded} error={list.error} freshId={freshId}
            rev={rev} settings={settings} filters={filters} onFilters={changeFilters} onMore={() => setLimit(l => l + PAGE)}
            onOpen={openViewer} onRecord={() => setView('recording')} onSettings={() => setView('settings')} onPatch={patch} onSettingsSaved={setSettings}
            onChanged={bump} onError={setNotice} />
        )}
        {view === 'recording' && <RecordingSetup display={settings?.display ?? null} onBack={() => setView('shelf')} onStarted={() => setView('shelf')} />}
        {view === 'settings' && settings && (
          <Settings settings={settings} onSaved={setSettings} onBack={() => setView('shelf')} />
        )}
      </section>
    </div>
  );
}
