import { useEffect, useState } from 'react';
import { Icon } from '../Icon';
import { native, type RecordingDevices, type RecordingOptions, type RecordingState, type RecordingTargets } from '../native';
import { errorMessage, useHold } from '../util';

export const DEFAULT_RECORDING: RecordingOptions = {
  kind: 'screen', display: '', window: null, fps: 60, quality: 'high', resolution: 0,
  cursor: true, countdown: true, systemAudio: false, microphone: null, camera: null,
  corner: 'BottomRight', cameraSize: 22,
};

export function useRecording() {
  const [state, setState] = useState<RecordingState>({ status: 'idle', elapsedMs: 0, frames: 0, error: null, options: null });
  useEffect(() => {
    let live = true;
    let off: (() => void) | undefined;
    let received = false;
    native.onRecordingState(value => { received = true; if (live) setState(value); }).then(async stop => {
      if (!live) { stop(); return; }
      off = stop;
      const value = await native.recordingState();
      if (live && !received) setState(value);
    }).catch(() => {});
    return () => { live = false; off?.(); };
  }, []);
  return state;
}

export function recordingTime(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
}

export function RecordingStatus({ state, onError }: { state: RecordingState; onError: (e: string) => void }) {
  const [pending, setPending] = useState(false);
  useEffect(() => setPending(false), [state.status]);
  if (state.status === 'idle') return null;
  const paused = state.status === 'paused';
  const ready = paused || state.status === 'recording';
  const run = async (action: 'pause' | 'resume' | 'stop') => {
    setPending(true);
    try { await native.controlRecording(action); }
    catch (e) { onError(errorMessage(e)); setPending(false); }
  };
  const options = state.options;
  return <div className="recording-status" role="region" aria-label="Recording controls">
    <div className="recording-readout">
      <span className="record-dot" data-paused={paused} aria-hidden="true" />
      <strong>{state.status.startsWith('countdown:') ? `Starting in ${state.status.split(':')[1]}…`
        : state.status === 'starting' ? 'Preparing…' : state.status === 'finishing' ? 'Saving recording…'
        : `${paused ? 'Paused' : 'Recording'} · ${recordingTime(state.elapsedMs)}`}</strong>
      <small>{options?.systemAudio ? 'System sound on' : 'System sound off'} · {options?.microphone ? 'Mic on' : 'Mic off'} · {options?.camera ? 'Camera on' : 'Camera off'}</small>
    </div>
    {ready && <button className="btn" disabled={pending} onClick={() => run(paused ? 'resume' : 'pause')}>{paused ? 'Resume' : 'Pause'}</button>}
    <button className="btn danger" disabled={pending || state.status === 'finishing'} onClick={() => run('stop')}>{ready ? 'Stop' : 'Cancel'}</button>
  </div>;
}

export function RecordingSetup({ display, onBack, onStarted }: { display: string | null; onBack: () => void; onStarted: () => void }) {
  useHold(true);
  const [options, setOptions] = useState<RecordingOptions>({ ...DEFAULT_RECORDING });
  const [targets, setTargets] = useState<RecordingTargets>({ displays: [], windows: [] });
  const [devices, setDevices] = useState<RecordingDevices | null>(null);
  const [mic, setMic] = useState(false);
  const [camera, setCamera] = useState(false);
  const [loadingDevices, setLoadingDevices] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const patch = (value: Partial<RecordingOptions>) => setOptions(o => ({ ...o, ...value }));
  useEffect(() => {
    let live = true;
    native.recordingTargets().then(value => {
      if (!live) return;
      setTargets(value);
      patch({ display: value.displays.find(d => d.id === display)?.id ?? value.displays[0]?.id ?? '', window: value.windows[0]?.id ?? null });
    }).catch(e => { if (live) setError(errorMessage(e)); });
    return () => { live = false; };
  }, [display]);
  useEffect(() => {
    if ((!mic && !camera) || devices) return;
    let live = true;
    setLoadingDevices(true);
    native.recordingDevices().then(value => { if (live) setDevices(value); })
      .catch(e => { if (live) setError(errorMessage(e)); })
      .finally(() => { if (live) setLoadingDevices(false); });
    return () => { live = false; };
  }, [mic, camera, devices]);
  const microphone = options.microphone ?? devices?.microphones[0]?.id ?? null;
  const webcam = options.camera ?? devices?.cameras[0]?.id ?? null;
  const valid = options.display && (options.kind !== 'window' || options.window) && (!mic || microphone) && (!camera || webcam);
  const start = async () => {
    if (!valid || busy) return;
    setBusy(true); setError('');
    try {
      await native.startRecording({ ...options, microphone: mic ? microphone : null, camera: camera ? webcam : null });
      onStarted();
    } catch (e) { setError(errorMessage(e)); setBusy(false); }
  };
  return <div className="view page">
    <div className="sheet">
      <header className="header">
        <button className="icon-btn" aria-label="Back to library" onClick={onBack} disabled={busy}><Icon name="back" /></button>
        <h1 className="title">Record video</h1>
      </header>
      <main className="body recording-setup">
        <p className="recording-intro">Screen only by default. Add sound or your camera when you need it.</p>
        <fieldset disabled={busy}>
          <legend>Capture</legend>
          <div className="seg" role="group" aria-label="Recording source">
            {(['screen', 'window', 'region'] as const).map(kind => <button key={kind} aria-pressed={options.kind === kind} onClick={() => patch({ kind })}>{kind[0].toUpperCase() + kind.slice(1)}</button>)}
          </div>
          {options.kind === 'window'
            ? <label>Window<select value={options.window ?? ''} onChange={e => patch({ window: e.target.value })}>
                {!targets.windows.length && <option value="">No available windows</option>}
                {targets.windows.map(w => <option key={w.id} value={w.id}>{w.name}</option>)}
              </select></label>
            : <label>Display<select value={options.display} onChange={e => patch({ display: e.target.value })}>
                {!targets.displays.length && <option value="">Loading displays…</option>}
                {targets.displays.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
              </select></label>}
          {options.kind === 'region' && <p className="recording-hint">Choose your area after pressing Start.</p>}
        </fieldset>
        <fieldset disabled={busy}>
          <legend>Sound & camera</legend>
          <label className="recording-check"><input type="checkbox" checked={options.systemAudio} onChange={e => patch({ systemAudio: e.target.checked })} />System sound</label>
          <label className="recording-check"><input type="checkbox" checked={mic} onChange={e => setMic(e.target.checked)} />Microphone</label>
          {mic && <label>Microphone device<select disabled={loadingDevices} value={microphone ?? ''} onChange={e => patch({ microphone: e.target.value })}>
            {!devices?.microphones.length && <option value="">{loadingDevices ? 'Finding microphones…' : 'No microphones available'}</option>}
            {devices?.microphones.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
          </select></label>}
          <label className="recording-check"><input type="checkbox" checked={camera} onChange={e => setCamera(e.target.checked)} />Webcam overlay</label>
          {camera && <>
            <label>Camera device<select disabled={loadingDevices} value={webcam ?? ''} onChange={e => patch({ camera: e.target.value })}>
              {!devices?.cameras.length && <option value="">{loadingDevices ? 'Finding cameras…' : 'No cameras available'}</option>}
              {devices?.cameras.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select></label>
            <div className="recording-columns">
              <label>Position<select value={options.corner} onChange={e => patch({ corner: e.target.value as RecordingOptions['corner'] })}>
                <option value="BottomRight">Bottom right</option><option value="BottomLeft">Bottom left</option>
                <option value="TopRight">Top right</option><option value="TopLeft">Top left</option>
              </select></label>
              <label>Size<select value={options.cameraSize} onChange={e => patch({ cameraSize: Number(e.target.value) as RecordingOptions['cameraSize'] })}>
                <option value={15}>Small</option><option value={22}>Medium</option><option value={30}>Large</option>
              </select></label>
            </div>
            <p className="recording-hint">The square camera inset is included in the saved video.</p>
          </>}
        </fieldset>
        <fieldset disabled={busy}>
          <legend>Video</legend>
          <div className="recording-columns">
            <label>Quality<select value={options.quality} onChange={e => patch({ quality: e.target.value as RecordingOptions['quality'] })}>
              <option value="standard">Standard</option><option value="high">High</option><option value="maximum">Maximum</option>
            </select></label>
            <label>Frame rate<select value={options.fps} onChange={e => patch({ fps: Number(e.target.value) as 30 | 60 })}>
              <option value={30}>30 fps</option><option value={60}>60 fps</option>
            </select></label>
          </div>
          <label>Resolution<select value={options.resolution} onChange={e => patch({ resolution: Number(e.target.value) as RecordingOptions['resolution'] })}>
            <option value={0}>Original size</option><option value={1080}>Up to 1080p</option><option value={720}>Up to 720p</option>
          </select></label>
          <label className="recording-check"><input type="checkbox" checked={options.cursor} onChange={e => patch({ cursor: e.target.checked })} />Show cursor</label>
          <label className="recording-check"><input type="checkbox" checked={options.countdown} onChange={e => patch({ countdown: e.target.checked })} />3-second countdown</label>
        </fieldset>
        <p className="recording-hint">Open the shelf to pause or stop. Captrain’s controls stay out of the recording.</p>
        {error && <p className="field-error" role="alert">{error}</p>}
      </main>
    </div>
    <footer className="recording-footer"><button className="btn" onClick={onBack} disabled={busy}>Cancel</button><button className="btn accent" disabled={!valid || busy} onClick={start}>{busy ? 'Starting…' : 'Start recording'}</button></footer>
  </div>;
}
