import { useEffect, useState } from 'react';
import { Icon } from '../Icon';
import { native, type Display, type Settings as SettingsData } from '../native';
import { errorMessage, kindLabel, quiet, useHold } from '../util';
import { findDuplicate, formatShortcut, recordShortcut } from './shortcut';
import { AppUpdates } from '../updates/AppUpdates';

type Action = keyof SettingsData['shortcuts'];
const ACTIONS: { id: Action; label: string }[] = [
  { id: 'region', label: 'Capture region' },
  { id: 'window', label: 'Capture window' },
  { id: 'screen', label: 'Capture screen' },
  { id: 'url', label: 'Save browser link' },
  { id: 'toggle', label: 'Toggle shelf' },
];

interface Props { settings: SettingsData; onSaved: (s: SettingsData) => void; onBack: () => void }

export function Settings({ settings, onSaved, onBack }: Props) {
  const [displays, setDisplays] = useState<Display[]>([]);
  const [storage, setStorage] = useState('');
  const [startup, setStartup] = useState<boolean | null>(null);
  useEffect(() => {
    let live = true;
    native.startup().then(value => { if (live) setStartup(value); }).catch(e => { if (live) setErrors(x => ({ ...x, startup: errorMessage(e) })); });
    return () => { live = false; };
  }, []);
  useEffect(() => {
    let live = true;
    native.storagePath().then(path => { if (live) setStorage(path); }).catch(() => {});
    return () => { live = false; };
  }, []);
  const [recording, setRecording] = useState<Action | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  useHold(true);

  useEffect(() => {
    let live = true;
    native.displays().then(d => { if (live) setDisplays(d); }).catch(e => { if (live) setErrors(x => ({ ...x, display: errorMessage(e) })); });
    return () => { live = false; };
  }, []);

  // A failed edit never replaces `settings`: the props only change from native's returned value.
  const apply = async (field: string, request: () => Promise<SettingsData>) => {
    setBusy(true);
    try {
      onSaved(await request());
      setErrors(({ [field]: _, ...rest }) => rest);
    } catch (e) {
      setErrors(x => ({ ...x, [field]: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (!recording) return;
    let live = true;
    let off: (() => void) | undefined;
    const accept = (shortcut: string) => {
      if (!live) return;
      setRecording(null);
      setHint(null);
      const dup = findDuplicate(settings.shortcuts, recording, shortcut);
      if (dup) {
        setErrors(x => ({ ...x, [recording]: `${formatShortcut(shortcut)} is already used by “${ACTIONS.find(a => a.id === dup)!.label}”.` }));
        return;
      }
      void apply(recording, () => native.updateSettings({ ...settings, shortcuts: { ...settings.shortcuts, [recording]: shortcut } }));
    };
    native.onRecorded(accept).then(async unlisten => {
      if (!live) { unlisten(); return; }
      off = unlisten;
      await native.recording(true);
    }).catch(e => { if (live) setHint(errorMessage(e)); });
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation(); // keep Escape and combos from reaching the shelf handlers
      if (e.repeat) return;
      if (e.code === 'Escape' && !e.ctrlKey && !e.altKey && !e.metaKey) { setRecording(null); setHint(null); return; }
      const result = recordShortcut(e);
      if (result.kind === 'pending') return;
      if (result.kind === 'error') { setHint(result.message); return; }
      accept(result.shortcut);
    };
    window.addEventListener('keydown', onKey, true);
    return () => { live = false; off?.(); quiet(native.recording(false)); window.removeEventListener('keydown', onKey, true); };
  }, [recording, settings]);

  return (
    <div className="view page"><div className="sheet">
      <header className="header">
        <button className="icon-btn" onClick={onBack} aria-label="Back"><Icon name="back" /></button>
        <h1 className="title">Settings</h1>
        <span className="brand-signature"><img src="/captrain.svg" width="22" height="22" alt="" />Captrain</span>
      </header>
      <main className="body settings">
        <AppUpdates />
        <section aria-labelledby="s-general">
          <h2 id="s-general">General</h2>
          <div className="field" role="radiogroup" aria-label="Appearance">
            <span className="label">Appearance</span>
            <div className="seg">
              {(['system', 'light', 'dark'] as const).map(theme => (
                <button key={theme} role="radio" aria-checked={settings.theme === theme} disabled={busy}
                  onClick={() => settings.theme !== theme && apply('theme', () => native.updateSettings({ ...settings, theme }))}>
                  {theme[0].toUpperCase() + theme.slice(1)}
                </button>
              ))}
            </div>
          </div>
          {errors.theme && <p className="field-error" role="alert">{errors.theme}</p>}
          <div className="field">
            <span className="label" id="l-startup">Start with Windows</span>
            <button className="switch" role="switch" aria-checked={startup ?? false} aria-labelledby="l-startup" disabled={busy || startup === null}
              onClick={async () => {
                setBusy(true);
                try { setStartup(await native.setStartup(!startup)); setErrors(({ startup: _, ...rest }) => rest); }
                catch (e) { setErrors(x => ({ ...x, startup: errorMessage(e) })); }
                finally { setBusy(false); }
              }} />
          </div>
          <p className="note">Open the shelf bar when you sign in. Enable this from your installed copy of Captrain.</p>
          {errors.startup && <p className="field-error" role="alert">{errors.startup}</p>}
        </section>
        <section aria-labelledby="s-display">
          <h2 id="s-display">Shelf</h2>
          <div className="field" role="radiogroup" aria-label="Screen edge">
            <span className="label">Edge</span>
            <div className="seg">
              {(['left', 'right'] as const).map(edge => (
                <button key={edge} role="radio" aria-checked={settings.edge === edge} disabled={busy}
                  onClick={() => settings.edge !== edge && apply('edge', () => native.updateSettings({ ...settings, edge }))}>
                  {edge === 'left' ? 'Left' : 'Right'}
                </button>
              ))}
            </div>
          </div>
          {errors.edge && <p className="field-error" role="alert">{errors.edge}</p>}
          <div className="field">
            <label className="label" htmlFor="display">Display</label>
            <select id="display" value={settings.display ?? ''} disabled={busy}
              onChange={e => apply('display', () => native.updateSettings({ ...settings, display: e.target.value || null }))}>
              <option value="">Primary display</option>
              {displays.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
              {settings.display && !displays.some(d => d.id === settings.display) && <option value={settings.display}>Unavailable display</option>}
            </select>
          </div>
          {errors.display && <p className="field-error" role="alert">{errors.display}</p>}
        </section>

        <section aria-labelledby="s-capture">
          <h2 id="s-capture">Capture</h2>
          <div className="field">
            <span className="label" id="l-instant">Capture instantly</span>
            <button className="switch" role="switch" aria-checked={settings.instantCapture} aria-labelledby="l-instant" disabled={busy}
              onClick={() => apply('instantCapture', () => native.updateSettings({ ...settings, instantCapture: !settings.instantCapture }))} />
          </div>
          <p className="note">{settings.instantCapture ? 'The Capture button starts the selected type right away.' : 'The Capture button asks which type to use first.'}</p>
          <div className="field" role="radiogroup" aria-label="Capture type">
            <span className="label">Capture type</span>
            <div className="seg">
              {(['region', 'window', 'screen'] as const).map(kind => (
                <button key={kind} role="radio" aria-checked={settings.captureKind === kind} disabled={busy}
                  onClick={() => settings.captureKind !== kind && apply('captureKind', () => native.updateSettings({ ...settings, captureKind: kind }))}>
                  {kindLabel(kind)}
                </button>
              ))}
            </div>
          </div>
          {errors.instantCapture && <p className="field-error" role="alert">{errors.instantCapture}</p>}
          {errors.captureKind && <p className="field-error" role="alert">{errors.captureKind}</p>}
        </section>

        <section aria-labelledby="s-save">
          <h2 id="s-save">Save to disk</h2>
          <p className="note">Every capture is also copied to this folder. Originals stay protected inside Captrain.</p>
          <p className="path" title={settings.captureFolder ?? undefined}>{settings.captureFolder ?? 'Off — captures stay in Captrain only'}</p>
          <div className="row-actions">
            <button className="btn" disabled={busy} onClick={() => apply('captureFolder', native.chooseCaptureFolder)}>{settings.captureFolder ? 'Change folder…' : 'Choose folder…'}</button>
            {settings.captureFolder && <button className="btn" disabled={busy} onClick={() => quiet(native.openCaptureFolder())}>Open</button>}
            {settings.captureFolder && <button className="btn" disabled={busy} onClick={() => apply('captureFolder', () => native.updateSettings({ ...settings, captureFolder: null }))}>Turn off</button>}
          </div>
          {errors.captureFolder && <p className="field-error" role="alert">{errors.captureFolder}</p>}
          {storage && <><label className="note" htmlFor="library-path">Protected library originals</label><input id="library-path" className="search" value={storage} readOnly title={storage} /></>}
        </section>

        <section aria-labelledby="s-keys">
          <h2 id="s-keys">Shortcuts</h2>
          <p className="note">Save browser link reads the address bar in Chrome, Edge or Brave without changing your clipboard. Only HTTP and HTTPS pages are supported.</p>
          {ACTIONS.map(({ id, label }) => (
            <div key={id}>
              <div className="field">
                <span className="label" id={`k-${id}`}>{label}</span>
                <button className={`keycap${recording === id ? ' recording' : ''}`} aria-labelledby={`k-${id}`} disabled={busy}
                  aria-description={recording === id ? 'Press the new shortcut, or Escape to cancel' : undefined}
                  onClick={() => { setHint(null); setRecording(recording === id ? null : id); setErrors(({ [id]: _, ...r }) => r); }}>
                  {recording === id ? 'Press keys…' : formatShortcut(settings.shortcuts[id])}
                </button>
              </div>
              {recording === id && hint && <p className="field-error" role="alert">{hint}</p>}
              {errors[id] && <p className="field-error" role="alert">{errors[id]}</p>}
            </div>
          ))}
          <p className="note" role="status">{recording ? 'Press the new combination. Esc cancels.' : 'Select a shortcut, then press the new combination.'}</p>
          <button className="btn" disabled={busy} onClick={() => { setRecording(null); void apply('defaults', native.defaults); }}>Restore default shortcuts</button>
          {errors.defaults && <p className="field-error" role="alert">{errors.defaults}</p>}
        </section>
      </main>
    </div></div>
  );
}
