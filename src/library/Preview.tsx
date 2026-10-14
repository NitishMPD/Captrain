import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Icon } from '../Icon';
import { native, type CaptureItem } from '../native';
import { errorMessage, formatTime, kindLabel, useHold } from '../util';

interface Props { item: CaptureItem; onBack: () => void; onDeleted: () => void; onError: (message: string) => void }

export function Preview({ item, onBack, onDeleted, onError }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [status, setStatus] = useState('');
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const failed = useRef(onError);
  failed.current = onError;
  useHold(confirming || busy);

  useEffect(() => {
    let live = true;
    setSrc(null);
    native.image(item.id).then(url => { if (live) setSrc(url); }).catch(e => { if (live) failed.current(errorMessage(e)); });
    return () => { live = false; setSrc(null); };
  }, [item.id]);

  useEffect(() => { root.current?.focus(); }, []);

  const run = async (task: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setStatus('');
    try { await task(); } catch (e) { onError(errorMessage(e)); } finally { setBusy(false); }
  };
  const copy = () => run(async () => { await native.copy(item.id); setStatus('Copied to clipboard'); });
  const save = () => run(async () => { if (await native.save(item.id)) setStatus('Saved'); });
  const remove = () => run(async () => {
    try { await native.delete(item.id); } catch (e) { setConfirming(false); throw e; }
    onDeleted();
  });

  const onKeyDown = (e: KeyboardEvent) => {
    if (confirming) {
      if (e.key === 'Escape') { e.preventDefault(); setConfirming(false); }
      return;
    }
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'c') { e.preventDefault(); void copy(); }
    else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 's') { e.preventDefault(); void save(); }
    else if (e.key === 'Delete') { e.preventDefault(); setConfirming(true); }
  };

  return (
    <div className="view" ref={root} tabIndex={-1} onKeyDown={onKeyDown}>
      <div className="view" inert={confirming}>
        <header className="header">
          <button className="icon-btn" onClick={onBack} aria-label="Back"><Icon name="back" /></button>
          <h1 className="title">{kindLabel(item.kind)} · {formatTime(item.createdAt)}</h1>
        </header>
        <main className="body preview">
          <div className="stage">
            {src ? <img src={src} alt={`${kindLabel(item.kind)} capture, ${item.width} by ${item.height} pixels`} draggable={false} />
              : <p className="empty">Loading preview…</p>}
          </div>
          <p className="dims">{item.width} × {item.height}<span className="status" role="status">{status}</span></p>
        </main>
        <footer className="footer actions">
          <button className="btn" onClick={copy} disabled={busy} title="Ctrl+C"><Icon name="copy" />Copy</button>
          <button className="btn" onClick={save} disabled={busy} title="Ctrl+Shift+S"><Icon name="save" />Save as</button>
          <button className="btn danger" onClick={() => setConfirming(true)} disabled={busy} title="Delete"><Icon name="trash" />Delete</button>
        </footer>
      </div>
      {confirming && (
        <div className="scrim">
          <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="del-title" aria-describedby="del-desc">
            <h2 id="del-title">Delete this capture?</h2>
            <p id="del-desc">The image and its thumbnail will be permanently removed from Captrain.</p>
            <div className="dialog-actions">
              <button className="btn" autoFocus onClick={() => setConfirming(false)}>Cancel</button>
              <button className="btn danger solid" onClick={remove} disabled={busy}>Delete</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
