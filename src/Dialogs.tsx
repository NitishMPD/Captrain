import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from 'react';
import { errorMessage, useHold } from './util';

/** `hold` keeps the shelf open; the viewer window has no shelf to hold. */
function Scrim({ onCancel, hold = true, children }: { onCancel: () => void; hold?: boolean; children: ReactNode }) {
  useHold(hold);
  const previous = useRef(document.activeElement);
  useEffect(() => () => { if (previous.current instanceof HTMLElement && previous.current.isConnected) previous.current.focus(); }, []);
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') { e.preventDefault(); onCancel(); }
    if (e.key === 'Tab') {
      const controls = [...e.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]')];
      const index = controls.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && index <= 0) { e.preventDefault(); controls.at(-1)?.focus(); }
      else if (!e.shiftKey && index === controls.length - 1) { e.preventDefault(); controls[0]?.focus(); }
    }
  };
  return <div className="scrim" onKeyDown={onKeyDown}>{children}</div>;
}

interface ConfirmProps { title: string; body: string; confirm: string; busy?: boolean; hold?: boolean; onConfirm: () => void; onCancel: () => void }

/** Destructive confirmation; Cancel holds initial focus. */
export function ConfirmDialog({ title, body, confirm, busy, hold, onConfirm, onCancel }: ConfirmProps) {
  return (
    <Scrim onCancel={onCancel} hold={hold}>
      <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dlg-title" aria-describedby="dlg-desc">
        <h2 id="dlg-title">{title}</h2>
        <p id="dlg-desc">{body}</p>
        <div className="dialog-actions">
          <button className="btn" autoFocus onClick={onCancel}>Cancel</button>
          <button className="btn danger solid" onClick={onConfirm} disabled={busy}>{confirm}</button>
        </div>
      </div>
    </Scrim>
  );
}

interface NameProps { title: string; initial?: string; submit: string; onSubmit: (name: string) => Promise<void>; onCancel: () => void }

/** Single-field form for naming or renaming a folder. */
export function NameDialog({ title, initial = '', submit, onSubmit, onCancel }: NameProps) {
  const [name, setName] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const clean = name.trim();

  const send = async (e: FormEvent) => {
    e.preventDefault();
    if (!clean || busy) return;
    setBusy(true);
    try { await onSubmit(clean); } catch (err) { setError(errorMessage(err)); setBusy(false); }
  };

  return (
    <Scrim onCancel={onCancel}>
      <form className="dialog" role="dialog" aria-modal="true" aria-labelledby="dlg-title" onSubmit={send}>
        <h2 id="dlg-title">{title}</h2>
        <input className="search name" autoFocus value={name} maxLength={40} aria-label="Folder name" aria-invalid={error !== null}
          aria-describedby={error ? 'dlg-error' : undefined} placeholder="Folder name"
          onChange={e => { setName(e.target.value); setError(null); }} onFocus={e => e.currentTarget.select()} />
        {error && <p id="dlg-error" className="field-error" role="alert">{error}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>Cancel</button>
          <button type="submit" className="btn accent" disabled={!clean || busy}>{submit}</button>
        </div>
      </form>
    </Scrim>
  );
}
