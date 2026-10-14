import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Icon } from '../Icon';
import { CaptureList } from '../library/CaptureList';
import { native, type CaptureItem, type CaptureKind, type Settings } from '../native';
import { formatShortcut } from '../settings/shortcut';
import { errorMessage, useHold } from '../util';

const KINDS: { kind: CaptureKind; label: string }[] = [
  { kind: 'region', label: 'Region' },
  { kind: 'window', label: 'Window' },
  { kind: 'screen', label: 'Screen' },
];

function CaptureMenu({ shortcuts, onError }: { shortcuts: Settings['shortcuts'] | null; onError: (m: string) => void }) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useHold(open);

  useEffect(() => {
    if (!open) return;
    wrap.current?.querySelector<HTMLElement>('[role=menuitem]')?.focus();
    const outside = (e: PointerEvent) => { if (!wrap.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);

  const pick = (kind: CaptureKind) => {
    setOpen(false);
    native.capture(kind).catch(e => onError(errorMessage(e)));
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (!open) return;
    if (e.key === 'Escape') { e.preventDefault(); setOpen(false); trigger.current?.focus(); return; }
    const items = [...wrap.current!.querySelectorAll<HTMLElement>('[role=menuitem]')];
    const i = items.indexOf(document.activeElement as HTMLElement);
    const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    if (step) { e.preventDefault(); items[(i + step + items.length) % items.length]?.focus(); }
  };

  return (
    <div className="menu-wrap" ref={wrap} onKeyDown={onKeyDown}>
      <button ref={trigger} className="btn primary" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Icon name="camera" />Capture<Icon name="chevron" />
      </button>
      {open && (
        <div className="menu" role="menu" aria-label="Capture type">
          {KINDS.map(({ kind, label }) => (
            <button key={kind} role="menuitem" onClick={() => pick(kind)}>
              <span>{label}</span>
              {shortcuts && <kbd>{formatShortcut(shortcuts[kind])}</kbd>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

interface Props {
  items: CaptureItem[];
  thumbs: Record<string, string>;
  loaded: boolean;
  error: string | null;
  freshId: string | null;
  shortcuts: Settings['shortcuts'] | null;
  onOpen: (item: CaptureItem) => void;
  onLibrary: () => void;
  onSettings: () => void;
  onError: (message: string) => void;
}

export function Shelf({ items, thumbs, loaded, error, freshId, shortcuts, onOpen, onLibrary, onSettings, onError }: Props) {
  return (
    <>
      <header className="header">
        <h1 className="title brand">Captrain</h1>
        <button className="icon-btn" onClick={onLibrary} aria-label="Search library"><Icon name="search" /></button>
        <button className="icon-btn" onClick={onSettings} aria-label="Settings"><Icon name="settings" /></button>
      </header>
      <main className="body">
        {error ? <p className="empty">{error}</p>
          : !loaded ? null
          : items.length === 0 ? (
            <p className="empty">No captures yet.{shortcuts && <> Press <kbd>{formatShortcut(shortcuts.region)}</kbd> or use Capture below.</>}</p>
          ) : <CaptureList items={items} thumbs={thumbs} freshId={freshId} onOpen={onOpen} />}
      </main>
      <footer className="footer">
        <CaptureMenu shortcuts={shortcuts} onError={onError} />
        <button className="btn" onClick={onLibrary}><Icon name="library" />Library</button>
      </footer>
    </>
  );
}
