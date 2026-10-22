import { useEffect, useRef, useState } from 'react';
import { Icon, type IconName } from '../Icon';
import { native, type RecordingOptions, type RecordingState, type RecordingTargets } from '../native';
import { errorMessage } from '../util';
import { DEFAULT_RECORDING, recordingTime, useRecording } from './Recording';

const KINDS: [RecordingOptions['kind'], string, IconName][] = [['screen', 'Screen', 'monitor'], ['window', 'Window', 'window'], ['region', 'Region', 'fit']];

function Toggle({ label, icon, on, busy, disabled, onClick }: { label: string; icon: IconName; on: boolean; busy?: boolean; disabled?: boolean; onClick: () => void }) {
  return <button className="island-btn" aria-label={label} title={label} aria-pressed={on} aria-busy={busy} disabled={disabled || busy} onClick={onClick}><Icon name={icon} /></button>;
}

/** Controls once Start is pressed: countdown/preparing, live timer with pause/stop, then saving. */
export function IslandActive({ state, pending, onAction }: { state: RecordingState; pending: boolean; onAction: (action: 'pause' | 'resume' | 'stop') => void }) {
  const paused = state.status === 'paused';
  const live = paused || state.status === 'recording';
  const o = state.options;
  const label = state.status.startsWith('countdown:') ? `Starting in ${state.status.split(':')[1]}`
    : state.status === 'starting' ? 'Preparing…' : state.status === 'finishing' ? 'Saving…' : paused ? 'Paused' : 'Recording';
  return <div className="island-row">
    <span className="island-dot" data-state={live ? (paused ? 'paused' : 'live') : 'idle'} aria-hidden="true" />
    <span role="status" className={live ? 'sr-only' : 'island-label'}>{label}</span>
    {live && <span className="island-time" aria-hidden="true">{recordingTime(state.elapsedMs)}</span>}
    {live && <span className="island-devices">
      {o?.systemAudio && <span title="System sound on"><Icon name="speaker" /></span>}
      {o?.microphone && <span title="Microphone on"><Icon name="mic" /></span>}
      {o?.camera && <span title="Camera on"><Icon name="camera" /></span>}
    </span>}
    {live && <button className="island-btn" aria-label={paused ? 'Resume recording' : 'Pause recording'} title={paused ? 'Resume' : 'Pause'} disabled={pending} onClick={() => onAction(paused ? 'resume' : 'pause')}><Icon name={paused ? 'play' : 'pause'} /></button>}
    {state.status !== 'finishing' && <button className="island-btn stop" aria-label={live ? 'Stop recording' : 'Cancel recording'} title={live ? 'Stop' : 'Cancel'} disabled={pending} onClick={() => onAction('stop')}><Icon name={live ? 'stop' : 'close'} /></button>}
  </div>;
}

export function RecordingIsland() {
  const state = useRecording();
  const pill = useRef<HTMLDivElement>(null);
  const [options, setOptions] = useState<RecordingOptions | null>(null);
  const [displays, setDisplays] = useState<RecordingTargets['displays']>([]);
  const [display, setDisplay] = useState('');
  const [windowName, setWindowName] = useState<string | null>(null);
  const [loadingDevice, setLoadingDevice] = useState<'microphone' | 'camera' | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  // Slides down from above the screen once the native window is placed, and back up before it closes.
  const [motion, setMotion] = useState<'hidden' | 'shown' | 'leaving'>('hidden');
  const saves = useRef<Promise<unknown>>(Promise.resolve());
  const loaded = useRef(false);
  const idle = state.status === 'idle';
  useEffect(() => setPending(false), [state.status]);
  const fail = (e: unknown) => setError(errorMessage(e));

  useEffect(() => {
    Promise.all([native.recordingPreferences().catch(e => { fail(e); return null; }), native.recordingTargets()])
      .then(([saved, targets]) => { setOptions(saved ?? { ...DEFAULT_RECORDING }); setDisplays(targets.displays); })
      .catch(fail);
  }, []);
  // Native bounds follow the pill so no invisible area blocks clicks under it.
  useEffect(() => {
    const el = pill.current!;
    const observer = new ResizeObserver(() => {
      const { width, height } = el.getBoundingClientRect();
      native.islandLayout(Math.ceil(width), Math.ceil(height)).then(id => {
        setDisplay(id);
        setMotion(m => m === 'hidden' ? 'shown' : m);
      }).catch(fail);
    });
    observer.observe(el);
    let live = true;
    let off: (() => void) | undefined;
    native.onIslandLeave(() => setMotion('leaving')).then(stop => { if (live) off = stop; else stop(); }).catch(() => {});
    return () => { live = false; observer.disconnect(); off?.(); };
  }, []);
  // The island's display is the screen to record; remember it with the other choices.
  useEffect(() => {
    if (options && display && options.display !== display) setOptions({ ...options, display });
  }, [options, display]);
  useEffect(() => {
    if (!options) return;
    if (!loaded.current) { loaded.current = true; return; }
    saves.current = saves.current.catch(() => {}).then(() => native.recordingPreferences(options));
    saves.current.catch(fail);
  }, [options]);
  useEffect(() => {
    if (options?.kind !== 'window' || !idle) return;
    const refresh = () => { native.islandWindow().then(setWindowName).catch(() => setWindowName(null)); };
    refresh();
    window.addEventListener('focus', refresh);
    return () => window.removeEventListener('focus', refresh);
  }, [options?.kind, idle]);
  useEffect(() => {
    if (!idle) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); native.closeIsland(false).catch(fail); } };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [idle]);

  const patch = (value: Partial<RecordingOptions>) => { setError(''); setOptions(o => o && { ...o, ...value }); };
  const device = async (kind: 'microphone' | 'camera') => {
    if (options?.[kind]) { patch({ [kind]: null }); return; }
    setLoadingDevice(kind);
    try {
      const found = await native.recordingDevices();
      const first = (kind === 'microphone' ? found.microphones : found.cameras)[0];
      if (first) patch({ [kind]: first.id }); else setError(kind === 'microphone' ? 'No microphone found.' : 'No camera found.');
    } catch (e) { fail(e); }
    finally { setLoadingDevice(null); }
  };
  const start = async () => {
    if (!options || !display || pending) return;
    setPending(true); setError('');
    try {
      await saves.current;
      await native.startRecording({ ...options, display, window: null });
    } catch (e) { fail(e); setPending(false); }
  };
  const control = (action: 'pause' | 'resume' | 'stop') => {
    setPending(true);
    native.controlRecording(action).catch(e => { fail(e); setPending(false); });
  };
  const moved = (p: Promise<string>) => { p.then(setDisplay).catch(fail); };
  const index = displays.findIndex(d => d.id === display);
  const screen = index >= 0 ? `Display ${index + 1}` : 'This display';
  const target = options?.kind === 'window' ? windowName ?? 'Switch to a window first' : options?.kind === 'region' ? `Region on ${screen}` : screen;

  return <div className="island" ref={pill} role="group" aria-label="Recording controls" data-motion={motion}
    onPointerDown={e => { if (e.button === 0 && !(e.target as Element).closest('button')) moved(native.islandDrag()); }}>
    {idle ? <div className="island-row">
      <button className="island-grip" aria-label={`Move controls, on ${screen}. Left and right arrows switch display.`} title="Drag to another display"
        onPointerDown={e => { if (e.button === 0) moved(native.islandDrag()); }}
        onKeyDown={e => { if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') { e.preventDefault(); moved(native.islandMove(e.key === 'ArrowLeft' ? -1 : 1)); } }}>
        <Icon name="grip" />
      </button>
      <span className="island-target" title={target}>{target}</span>
      <div className="island-seg" role="group" aria-label="Record">
        {KINDS.map(([kind, label, icon]) => <Toggle key={kind} label={label} icon={icon} on={options?.kind === kind} disabled={!options} onClick={() => patch({ kind })} />)}
      </div>
      <span className="island-sep" aria-hidden="true" />
      <Toggle label="System sound" icon="speaker" on={!!options?.systemAudio} disabled={!options} onClick={() => patch({ systemAudio: !options!.systemAudio })} />
      <Toggle label="Microphone" icon="mic" on={!!options?.microphone} busy={loadingDevice === 'microphone'} disabled={!options} onClick={() => { void device('microphone'); }} />
      <Toggle label="Webcam overlay" icon="camera" on={!!options?.camera} busy={loadingDevice === 'camera'} disabled={!options} onClick={() => { void device('camera'); }} />
      <span className="island-sep" aria-hidden="true" />
      <button className="island-btn" aria-label="All recording settings" title="All recording settings" onClick={() => native.closeIsland(true).catch(fail)}><Icon name="settings" /></button>
      <button className="island-record" autoFocus disabled={!options || !display || pending || (options.kind === 'window' && !windowName)} onClick={() => { void start(); }}>
        <span className="island-dot" data-state="live" aria-hidden="true" />{pending ? 'Starting…' : 'Record'}
      </button>
      <button className="island-btn" aria-label="Close recording controls" title="Close (Esc)" onClick={() => native.closeIsland(false).catch(fail)}><Icon name="close" /></button>
    </div> : <IslandActive state={state} pending={pending} onAction={control} />}
    {error && <p className="island-error" role="alert">{error}</p>}
  </div>;
}
