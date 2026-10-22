import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
vi.mock('../native', () => ({ native: {} }));
import { DEFAULT_RECORDING, RecordingSetup, RecordingStatus, recordingTime, restoreRecording, RecordingCountdown } from './Recording';
import { IslandActive, RecordingIsland } from './Island';

describe('recording controls', () => {
  it('starts with all devices off and requires an explicit start', () => {
    expect(DEFAULT_RECORDING.systemAudio).toBe(false);
    expect(DEFAULT_RECORDING.microphone).toBeNull();
    expect(DEFAULT_RECORDING.camera).toBeNull();
    const html = renderToStaticMarkup(<RecordingSetup display={null} onBack={() => {}} onStarted={() => {}} />);
    expect(html.match(/type="checkbox" checked=""/g)).toHaveLength(2); // cursor and countdown only
    expect(html).toContain('Start recording');
    expect(html).toContain('Screen only by default');
  });
  it('restores preferences, falls back from disconnected displays and refreshes window handles', () => {
    const targets = { displays: [{ id: 'primary', name: 'Display 1' }, { id: 'secondary', name: 'Display 2' }], windows: [{ id: 'new-window', name: 'Editor' }] };
    const saved = { ...DEFAULT_RECORDING, display: 'secondary', window: 'stale-window', fps: 30 as const, systemAudio: true, microphone: 'saved-mic' };
    expect(restoreRecording(saved, targets, 'primary')).toEqual({ ...saved, window: 'new-window' });
    expect(restoreRecording({ ...saved, display: 'unplugged' }, targets, 'primary').display).toBe('primary');
    expect(restoreRecording(null, targets, null).display).toBe('primary');
    expect(restoreRecording(saved, { displays: [], windows: [] }, null)).toMatchObject({ display: '', window: null });
  });
  it('labels the target screen in the countdown and keeps cancellation available', () => {
    vi.stubGlobal('location', { search: '?countdown=1&target=Display+2+-+Selected+region' });
    try {
      const html = renderToStaticMarkup(<RecordingCountdown />);
      expect(html).toContain('Display 2 - Selected region');
      expect(html).toContain('aria-live="assertive">3</span>');
      expect(html).toContain('Cancel');
      expect(html).toContain('Esc');
    } finally { vi.unstubAllGlobals(); }
  });
  it('shows pause/resume and the actual enabled device indicators', () => {
    const html = renderToStaticMarkup(<RecordingStatus state={{ status: 'paused', elapsedMs: 65000, frames: 100,
      error: null, options: { ...DEFAULT_RECORDING, systemAudio: true } }} onError={() => {}} />);
    expect(html).toContain('Paused');
    expect(html).toContain('01:05');
    expect(html).toContain('Resume');
    expect(html).toContain('System sound on');
    expect(html).toContain('Mic off');
    expect(html).toContain('Camera off');
    expect(html).toContain('Stop');
    expect(recordingTime(3_600_000)).toBe('60:00');
  });
  it('island offers record, move and close before starting, then the right controls per state', () => {
    const ready = renderToStaticMarkup(<RecordingIsland />);
    expect(ready).toContain('Record');
    expect(ready).toContain('Move controls');
    expect(ready).toContain('Close recording controls');
    const view = (status: string) => renderToStaticMarkup(<IslandActive pending={false} onAction={() => {}}
      state={{ status, elapsedMs: 65000, frames: 0, error: null, options: { ...DEFAULT_RECORDING, microphone: 'mic' } }} />);
    const live = view('recording');
    expect(live).toContain('01:05');
    expect(live).toContain('Pause recording');
    expect(live).toContain('Stop recording');
    expect(live).toContain('Microphone on');
    expect(live).not.toContain('System sound on');
    expect(view('paused')).toContain('Resume recording');
    const countdown = view('countdown:2');
    expect(countdown).toContain('Starting in 2');
    expect(countdown).toContain('Cancel recording');
    expect(countdown).not.toContain('Pause');
    expect(view('finishing')).toContain('Saving…');
    expect(view('finishing')).not.toContain('Stop');
  });
});
