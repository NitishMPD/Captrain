import { describe, expect, it, vi } from 'vitest';
import { createUpdates } from './controller';

function fixture(supported = true) {
  const api = {
    check: vi.fn(async () => ({ version: '0.1.2', notes: 'Changes' })),
    download: vi.fn(async (progress: (n: number | null) => void) => { progress(120); }),
    install: vi.fn(async () => {}),
  };
  return { api, updates: createUpdates(api, supported) };
}

describe('GitHub update lifecycle', () => {
  it('disables updates in development', async () => {
    const { api, updates } = fixture(false);
    await updates.check();
    await updates.download();
    await updates.install();
    expect(updates.getState().status).toBe('unsupported');
    expect(api.check).not.toHaveBeenCalled();
    expect(api.download).not.toHaveBeenCalled();
    expect(api.install).not.toHaveBeenCalled();
  });

  it('requires explicit download and install and keeps a downloaded update', async () => {
    const { api, updates } = fixture();
    await updates.install();
    expect(api.install).not.toHaveBeenCalled();
    await updates.check();
    expect(updates.getState().status).toBe('available');
    expect(api.download).not.toHaveBeenCalled();
    await updates.download();
    await updates.check();
    expect(api.check).toHaveBeenCalledTimes(1);
    expect(updates.getState().percent).toBe(100);
    expect(api.install).not.toHaveBeenCalled();
    await updates.install();
    expect(api.install).toHaveBeenCalledTimes(1);
  });

  it('coalesces concurrent checks', async () => {
    const { api, updates } = fixture();
    await Promise.all([updates.check(), updates.check(), updates.check()]);
    expect(api.check).toHaveBeenCalledTimes(1);
  });

  it('allows retry after network, download, or installation errors', async () => {
    const { api, updates } = fixture();
    api.check.mockRejectedValueOnce(new Error('Offline'));
    await updates.check();
    expect(updates.getState()).toMatchObject({ status: 'error', error: 'Offline' });
    await updates.check();
    api.download.mockRejectedValueOnce(new Error('Bad signature'));
    await updates.download();
    expect(updates.getState()).toMatchObject({ status: 'available', error: 'Bad signature' });
    await updates.download();
    api.install.mockRejectedValueOnce(new Error('Capture active'));
    await updates.install();
    expect(updates.getState()).toMatchObject({ status: 'downloaded', error: 'Capture active' });
    await updates.install();
    expect(updates.getState()).toMatchObject({ status: 'installing', error: null });
  });

  it('reports no newer version without downloading', async () => {
    const api = { check: async () => null, download: vi.fn(), install: vi.fn() };
    const updates = createUpdates(api, true);
    await updates.check();
    expect(updates.getState().status).toBe('up-to-date');
    expect(api.download).not.toHaveBeenCalled();
  });
});
