import { describe, expect, it } from 'vitest';
import { findDuplicate, formatShortcut, recordShortcut } from './shortcut';

const key = (code: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}) =>
  recordShortcut({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

describe('recordShortcut', () => {
  it('waits while only modifiers are held', () => {
    expect(key('ControlLeft', { ctrlKey: true })).toEqual({ kind: 'pending' });
  });
  it('records modifier combinations in a fixed order', () => {
    expect(key('Digit1', { altKey: true, ctrlKey: true })).toEqual({ kind: 'ok', shortcut: 'Ctrl+Alt+1' });
    expect(key('Space', { metaKey: true, shiftKey: true, ctrlKey: true })).toEqual({ kind: 'ok', shortcut: 'Ctrl+Shift+Super+Space' });
  });
  it('rejects unmodified and shift-only letters or digits', () => {
    expect(key('KeyA').kind).toBe('error');
    expect(key('Digit1', { shiftKey: true }).kind).toBe('error');
  });
  it('allows bare function keys and rejects unsupported keys', () => {
    expect(key('F9')).toEqual({ kind: 'ok', shortcut: 'F9' });
    expect(key('Escape', { ctrlKey: true }).kind).toBe('error');
  });
});

describe('shortcut helpers', () => {
  it('finds duplicates case-insensitively, ignoring the edited action', () => {
    const s = { region: 'Ctrl+Alt+1', window: 'Ctrl+Alt+2', screen: 'Ctrl+Alt+3' };
    expect(findDuplicate(s, 'screen', 'ctrl+alt+1')).toBe('region');
    expect(findDuplicate(s, 'region', 'Ctrl+Alt+1')).toBeUndefined();
  });
  it('labels Super as Win', () => expect(formatShortcut('Ctrl+Super+K')).toBe('Ctrl+Win+K'));
});
