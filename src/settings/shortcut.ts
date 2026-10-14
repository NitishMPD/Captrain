export type Recorded =
  | { kind: 'pending' }
  | { kind: 'error'; message: string }
  | { kind: 'ok'; shortcut: string };

export interface KeyInput { code: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }

const NAMED: Record<string, string> = {
  Space: 'Space', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
  Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown', Insert: 'Insert', Delete: 'Delete',
};
const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  return NAMED[code] ?? null;
}

/** Turns a keydown into a global shortcut string such as "Ctrl+Alt+1". */
export function recordShortcut(e: KeyInput): Recorded {
  if (MODIFIER_CODES.test(e.code)) return { kind: 'pending' };
  const key = keyName(e.code);
  if (!key) return { kind: 'error', message: 'That key cannot be used in a shortcut.' };
  const isFunctionKey = /^F\d+$/.test(key);
  if (!isFunctionKey && !(e.ctrlKey || e.altKey || e.metaKey)) {
    return { kind: 'error', message: 'Include Ctrl, Alt or Win so the shortcut does not block normal typing.' };
  }
  const parts = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super', key];
  return { kind: 'ok', shortcut: parts.filter(Boolean).join('+') };
}

export const formatShortcut = (s: string) => s.replace(/\bSuper\b/g, 'Win');

/** Returns the other action already using this shortcut, if any. */
export function findDuplicate<K extends string>(shortcuts: Record<K, string>, action: K, shortcut: string): K | undefined {
  const target = shortcut.toLowerCase();
  return (Object.keys(shortcuts) as K[]).find(k => k !== action && shortcuts[k].toLowerCase() === target);
}
