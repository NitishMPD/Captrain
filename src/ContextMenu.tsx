import { useLayoutEffect, useEffect, useRef, useState, type KeyboardEvent, type MouseEvent as ReactMouseEvent } from 'react';
import { useHold } from './util';

export interface MenuItem {
  label: string;
  hint?: string;
  onSelect?: () => void;
  submenu?: MenuItem[];
  /** Defined → radio/checkbox semantics. */
  checked?: boolean;
  danger?: boolean;
  disabled?: boolean;
  separatorBefore?: boolean;
}

interface Anchor { x: number; y: number; up: boolean; items: MenuItem[]; returnTo: HTMLElement | null }

const MARGIN = 8;

function Menu({ anchor, onClose }: { anchor: Anchor; onClose: () => void }) {
  const [stack, setStack] = useState<MenuItem[][]>([anchor.items]);
  const [pos, setPos] = useState({ left: anchor.x, top: anchor.y });
  const ref = useRef<HTMLDivElement>(null);
  const items = stack[stack.length - 1];
  useHold(true);

  // Measured before paint so the menu never flashes outside the viewport.
  useLayoutEffect(() => {
    const box = ref.current!.getBoundingClientRect();
    const left = Math.max(MARGIN, Math.min(anchor.x, innerWidth - box.width - MARGIN));
    const wanted = anchor.up ? anchor.y - box.height - MARGIN : anchor.y;
    setPos({ left, top: Math.max(MARGIN, Math.min(wanted, innerHeight - box.height - MARGIN)) });
  }, [anchor, stack.length]);

  useEffect(() => { ref.current?.querySelector<HTMLElement>('[role^=menuitem]:not(:disabled)')?.focus(); }, [stack.length]);

  useEffect(() => {
    const outside = (e: PointerEvent) => { if (!ref.current?.contains(e.target as Node)) onClose(); };
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  const choose = (item: MenuItem) => {
    if (item.submenu) { setStack(s => [...s, item.submenu!]); return; }
    onClose();
    item.onSelect?.();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault(); // keeps the shelf's own Escape handler from collapsing it
      if (stack.length > 1) setStack(s => s.slice(0, -1)); else onClose();
      return;
    }
    if (e.key === 'Tab') { e.preventDefault(); onClose(); return; }
    if (e.key === 'ArrowLeft' && stack.length > 1) { e.preventDefault(); setStack(s => s.slice(0, -1)); return; }
    const buttons = [...ref.current!.querySelectorAll<HTMLElement>('[role^=menuitem]:not(:disabled)')];
    const i = buttons.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowRight' && buttons[i]?.getAttribute('aria-haspopup') === 'menu') { e.preventDefault(); buttons[i].click(); return; }
    const target = e.key === 'ArrowDown' ? (i + 1) % buttons.length : e.key === 'ArrowUp' ? (i - 1 + buttons.length) % buttons.length
      : e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : -1;
    if (target >= 0) { e.preventDefault(); buttons[target]?.focus(); }
  };

  return (
    <div className="menu" role="menu" ref={ref} style={pos} onKeyDown={onKeyDown} onContextMenu={e => e.preventDefault()}>
      {stack.length > 1 && (
        <button role="menuitem" className="menu-back" onClick={() => setStack(s => s.slice(0, -1))}>‹ Back</button>
      )}
      {items.map(item => (
        <div key={item.label} role="none" className={item.separatorBefore ? 'menu-sep' : undefined}>
          <button role={item.checked === undefined ? 'menuitem' : 'menuitemradio'} aria-checked={item.checked}
            aria-haspopup={item.submenu ? 'menu' : undefined} disabled={item.disabled}
            className={item.danger ? 'danger' : undefined} onClick={() => choose(item)}>
            <span>{item.checked !== undefined && <span className="menu-check" aria-hidden="true">{item.checked ? '✓' : ''}</span>}{item.label}</span>
            {item.hint ? <kbd>{item.hint}</kbd> : item.submenu ? <span aria-hidden="true">›</span> : null}
          </button>
        </div>
      ))}
    </div>
  );
}

/**
 * One menu at a time. `open` is for buttons (anchored, optionally above) and
 * `bind` returns right-click / Menu key / Shift+F10 handlers for a row.
 * Closing returns focus to the element that opened it.
 */
export function useContextMenu() {
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  const close = useRef(() => {});
  close.current = () => { anchor?.returnTo?.focus(); setAnchor(null); };
  const onClose = useRef(() => close.current()).current;

  const open = (items: MenuItem[], x: number, y: number, up: boolean, returnTo: HTMLElement | null) =>
    setAnchor({ items, x, y, up, returnTo });

  const bind = (items: () => MenuItem[]) => ({
    onContextMenu: (e: ReactMouseEvent<HTMLElement>) => {
      e.preventDefault();
      e.stopPropagation();
      open(items(), e.clientX, e.clientY, false, e.currentTarget);
    },
    onKeyDown: (e: KeyboardEvent<HTMLElement>) => {
      if (e.key !== 'ContextMenu' && !(e.shiftKey && e.key === 'F10')) return;
      e.preventDefault();
      const r = e.currentTarget.getBoundingClientRect();
      open(items(), r.left + Math.min(r.width / 2, 120), r.top + r.height / 2, false, e.currentTarget);
    },
  });

  /** Popover anchored above a button. */
  const openAbove = (items: MenuItem[], button: HTMLElement) => {
    const r = button.getBoundingClientRect();
    open(items, r.left, r.top, true, button);
  };

  return { menu: anchor && <Menu anchor={anchor} onClose={onClose} />, isOpen: anchor !== null, bind, openAbove };
}
