import { Channel, invoke } from '@tauri-apps/api/core';
import { useSyncExternalStore } from 'react';
import { createUpdates, type Available } from './controller';

export const updates = createUpdates({
  check: () => invoke<Available | null>('check_update'),
  download: onProgress => {
    const progress = new Channel<number | null>();
    progress.onmessage = onProgress;
    return invoke('download_update', { progress });
  },
  install: () => invoke('install_update'),
}, !import.meta.env.DEV);

export const useUpdates = () => useSyncExternalStore(updates.subscribe, updates.getState);

export function startUpdateChecks() {
  if (import.meta.env.DEV) return () => {};
  const first = setTimeout(() => { void updates.check(); }, 10_000);
  const periodic = setInterval(() => { void updates.check(); }, 6 * 60 * 60 * 1000);
  return () => { clearTimeout(first); clearInterval(periodic); };
}
