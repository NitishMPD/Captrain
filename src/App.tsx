import { useCallback, useEffect, useRef, useState } from 'react';
import { Library } from './library/Library';
import { Preview } from './library/Preview';
import { useCaptures } from './library/useCaptures';
import { native, type CaptureItem, type Settings as SettingsData, type ShelfState } from './native';
import { Settings } from './settings/Settings';
import { Shelf } from './shelf/Shelf';
import { errorMessage, quiet } from './util';

type View =
  | { name: 'recent' }
  | { name: 'library' }
  | { name: 'settings' }
  | { name: 'preview'; item: CaptureItem; back: 'recent' | 'library' };

const RECENT = 20;

/** The first frame commits the thumbnail; the second follows its first paint. */
function afterPaint(run: () => void) {
  requestAnimationFrame(() => requestAnimationFrame(run));
}

export function App() {
  const [shelf, setShelf] = useState<ShelfState>({ mode: 'collapsed', hovered: false, error: null, reveal: 0 });
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [view, setView] = useState<View>({ name: 'recent' });
  const [rev, setRev] = useState(0);
  const [freshId, setFreshId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const mode = useRef(shelf.mode);
  const reveal = useRef(shelf.reveal);
  const awaitingItem = useRef(false);

  const armTimer = useCallback(() => {
    const generation = reveal.current;
    afterPaint(() => { if (mode.current === 'temporary') quiet(native.shelf('ready', generation)); });
  }, []);
  const onSettled = useCallback(() => {
    if (!awaitingItem.current) return;
    awaitingItem.current = false;
    armTimer();
  }, [armTimer]);
  const recent = useCaptures('', 0, RECENT, rev, onSettled);

  useEffect(() => {
    let disposed = false;
    const unlisten: (() => void)[] = [];
    (async () => {
      const [offState, offCapture] = await Promise.all([
        native.onState(s => {
          const previous = mode.current;
          mode.current = s.mode;
          reveal.current = s.reveal;
          setShelf(s);
          if (s.mode === 'temporary' && previous !== 'temporary' && !awaitingItem.current) armTimer();
        }),
        native.onCapture(item => {
          awaitingItem.current = true;
          setFreshId(item.id);
          setRev(r => r + 1);
        }),
      ]);
      if (disposed) { offState(); offCapture(); return; }
      unlisten.push(offState, offCapture);
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

  // A collapsed shelf always reopens on the recent list showing the newest capture.
  useEffect(() => {
    if (shelf.mode !== 'collapsed') return;
    setView({ name: 'recent' });
    setFreshId(null);
  }, [shelf.mode]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.defaultPrevented) quiet(native.shelf('collapse'));
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const error = notice ?? shelf.error;
  const dismiss = () => { setNotice(null); if (shelf.error) quiet(native.shelf('clearError')); };
  const bump = () => setRev(r => r + 1);

  if (shelf.mode === 'capturing') return null;

  const edge = settings?.edge ?? 'right';
  const root = {
    className: `root ${edge}`,
    'data-mode': shelf.mode,
    onPointerEnter: () => quiet(native.shelf('enter')),
    onPointerLeave: () => quiet(native.shelf('leave')),
  };

  if (shelf.mode === 'collapsed') {
    return (
      <div {...root}>
        <button className="bar" data-hovered={shelf.hovered} aria-label="Open Captrain shelf" onClick={() => quiet(native.shelf('open'))}>
          <span className="bar-line" />
        </button>
      </div>
    );
  }

  const open = (item: CaptureItem, back: 'recent' | 'library') => setView({ name: 'preview', item, back });
  const backFrom = (v: Extract<View, { name: 'preview' }>) => setView(v.back === 'library' ? { name: 'library' } : { name: 'recent' });

  return (
    <div {...root} onPointerDownCapture={() => { if (shelf.mode === 'temporary') quiet(native.shelf('interact')); }}>
      <section className="panel" aria-label="Captrain shelf">
        {error && (
          <div className="banner" role="alert">
            <span>{error}</span>
            <button className="icon-btn" onClick={dismiss} aria-label="Dismiss message">×</button>
          </div>
        )}
        {view.name === 'recent' && (
          <Shelf items={recent.items} thumbs={recent.thumbs} loaded={recent.loaded} error={recent.error} freshId={freshId}
            shortcuts={settings?.shortcuts ?? null} onOpen={item => open(item, 'recent')}
            onLibrary={() => setView({ name: 'library' })} onSettings={() => setView({ name: 'settings' })} onError={setNotice} />
        )}
        {view.name === 'library' && (
          <Library rev={rev} onBack={() => setView({ name: 'recent' })} onOpen={item => open(item, 'library')} />
        )}
        {view.name === 'settings' && settings && (
          <Settings settings={settings} onSaved={setSettings} onBack={() => setView({ name: 'recent' })} />
        )}
        {view.name === 'preview' && (
          <Preview key={view.item.id} item={view.item} onBack={() => backFrom(view)} onError={setNotice}
            onDeleted={() => { bump(); backFrom(view); }} />
        )}
      </section>
    </div>
  );
}
