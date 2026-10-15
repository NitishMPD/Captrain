import { useEffect } from 'react';
import { native, type CaptureKind } from './native';

export const errorMessage = (e: unknown): string =>
  typeof e === 'string' ? e : e instanceof Error ? e.message : (e as { message?: string } | null)?.message ?? 'Something went wrong.';

/** Fire-and-forget native call whose failure must not become an unhandled rejection. */
export const quiet = (p: Promise<unknown>) => { p.catch(() => {}); };

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
export const formatTime = (iso: string) => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? 'Unknown time' : dateFormat.format(d);
};
const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto', style: 'short' });
const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [['year', 31536000], ['month', 2592000], ['day', 86400], ['hour', 3600], ['minute', 60]];
export const relativeTime = (iso: string, now = Date.now()) => {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return 'Unknown time';
  const seconds = Math.round((t - now) / 1000);
  const [unit, size] = UNITS.find(([, s]) => Math.abs(seconds) >= s) ?? ['second', 1];
  return Math.abs(seconds) < 45 ? 'Just now' : relative.format(Math.round(seconds / size), unit);
};
export const kindLabel =(k: CaptureKind | 'url' | 'video') => k === 'url' ? 'Link' : k === 'video' ? 'Video' : k[0].toUpperCase() + k.slice(1);

/** Keeps the shelf from auto-dismissing while a menu, dialog or settings is open. */
export function useHold(active: boolean) {
  useEffect(() => {
    if (!active || document.documentElement.dataset.surface === 'viewer') return;
    quiet(native.shelf('hold'));
    return () => quiet(native.shelf('release'));
  }, [active]);
}
