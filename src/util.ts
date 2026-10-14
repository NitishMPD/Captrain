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
export const kindLabel = (k: CaptureKind) => k[0].toUpperCase() + k.slice(1);

/** Keeps the shelf from auto-dismissing while a menu, dialog or settings is open. */
export function useHold(active: boolean) {
  useEffect(() => {
    if (!active) return;
    quiet(native.shelf('hold'));
    return () => quiet(native.shelf('release'));
  }, [active]);
}
