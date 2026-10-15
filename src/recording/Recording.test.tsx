import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
vi.mock('../native', () => ({ native: {} }));
import { DEFAULT_RECORDING, RecordingSetup, RecordingStatus, recordingTime } from './Recording';

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
});
