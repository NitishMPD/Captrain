import { useCallback, useEffect, useRef, useState, type PointerEvent } from 'react';
import { ConfirmDialog } from '../Dialogs';
import { useContextMenu } from '../ContextMenu';
import { Icon } from '../Icon';
import { native, type CaptureItem, type Folder, type ViewerContext } from '../native';
import { locate, MAX_PAGE } from '../library/paging';
import { errorMessage, formatTime, kindLabel, quiet } from '../util';
import { clampPan, fitTransform, indexAfterDelete, stepIndex, zoomAt, type Size, type Transform } from './transform';

const PREFETCH = 5;

/** Full-window image viewer: navigates the shelf's current library context, not just the recent page. */
export function Viewer() {
  const contextMenu = useContextMenu();
  const [ctx, setCtx] = useState<ViewerContext | null>(null);
  const [items, setItems] = useState<CaptureItem[]>([]);
  const [total, setTotal] = useState(0);
  const [index, setIndex] = useState(0);
  const [ready, setReady] = useState(false);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [src, setSrc] = useState<string | null>(null);
  const [view, setView] = useState<Transform | null>(null); // null → fit
  const [stage, setStage] = useState<Size>({ width: 0, height: 0 });
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState<string | null>(null);
  const stageEl = useRef<HTMLDivElement>(null);
  const loading = useRef(false);
  const drag = useRef<{ x: number; y: number; from: Transform } | null>(null);
  const item = items[index];
  const size: Size = { width: item?.width ?? 0, height: item?.height ?? 0 };
  const current = view ?? fitTransform(stage, size);
  const latest = useRef({ current, stage, size });
  latest.current = { current, stage, size };

  const loadPage = useCallback(async (context: ViewerContext, offset: number) => {
    const page = await native.list(context.query, offset, MAX_PAGE, context.folderId, context.exportedOnly, true);
    setTotal(page.total);
    return page.items;
  }, []);

  // Read pages from the start until the opened capture is found, so previous/next span the whole context.
  useEffect(() => {
    let live = true;
    (async () => {
      const [context, all] = await Promise.all([native.viewerContext(), native.folders().catch(() => [] as Folder[])]);
      if (!live) return;
      setCtx(context);
      setFolders(all);
      const found = await locate((o, l) => native.list(context.query, o, l, context.folderId, context.exportedOnly, true), context.id);
      if (!live) return;
      setItems(found.items);
      setTotal(found.total);
      setIndex(Math.max(0, found.index));
      setReady(true);
    })().catch(e => { if (live) { setError(errorMessage(e)); setReady(true); } });
    return () => { live = false; };
  }, [loadPage]);

  useEffect(() => {
    if (!ctx || loading.current || items.length >= total || index < items.length - PREFETCH) return;
    loading.current = true;
    loadPage(ctx, items.length)
      .then(page => setItems(prev => (prev.length === items.length ? [...prev, ...page] : prev)))
      .catch(e => setError(errorMessage(e)))
      .finally(() => { loading.current = false; });
  }, [ctx, items.length, total, index, loadPage]);

  // Each capture opens fitted and clears its own image, status and confirmation.
  useEffect(() => {
    if (!item) return;
    let live = true;
    setSrc(null);
    setView(null);
    setStatus('');
    native.image(item.id).then(url => { if (live) setSrc(url); }).catch(e => { if (live) setError(errorMessage(e)); });
    return () => { live = false; };
  }, [item?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const el = stageEl.current;
    if (!el) return;
    const measure = () => setStage({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ready, item === undefined]);

  const zoom = useCallback((factor: number, point = { x: 0, y: 0 }) => {
    const { current: t, stage: s, size: z } = latest.current;
    setView(zoomAt(t, factor, point, s, z));
  }, []);

  // React registers wheel listeners as passive, so the page-zoom guard needs a native one.
  useEffect(() => {
    const el = stageEl.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      zoom(Math.exp(-e.deltaY * 0.0015), { x: e.clientX - r.left - r.width / 2, y: e.clientY - r.top - r.height / 2 });
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [zoom, ready, item === undefined]);

  const go = (delta: number) => {
    const next = stepIndex(index, delta, items.length);
    if (next >= 0 && next !== index) setIndex(next);
  };
  const actual = () => setView(clampPan({ scale: 1, x: 0, y: 0 }, stage, size));
  const close = () => quiet(native.closeViewer());

  const run = async (task: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setStatus('');
    setError(null);
    try { await task(); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  };
  const copy = () => item && run(async () => { await native.copy(item.id); setStatus('Copied to clipboard'); });
  const save = () => item && run(async () => { if (await native.save(item.id)) setStatus('Saved'); });
  const move = (folderId: string | null) => item && run(async () => {
    await native.moveCapture(item.id, folderId);
    setItems(prev => prev.map(i => (i.id === item.id ? { ...i, folderId } : i)));
    setStatus(folderId ? `Moved to ${folders.find(f => f.id === folderId)?.name ?? 'folder'}` : 'Removed from folder');
  });
  const remove = () => item && run(async () => {
    try { await native.delete(item.id); } catch (e) { setConfirming(false); throw e; }
    const rest = items.filter(i => i.id !== item.id);
    setConfirming(false);
    if (rest.length === 0 && total <= 1) { close(); return; }
    setItems(rest);
    setTotal(t => t - 1);
    setIndex(Math.max(0, indexAfterDelete(index, rest.length)));
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (confirming || e.defaultPrevented || (e.target instanceof HTMLSelectElement)) return;
      if (e.key === 'Escape') close();
      else if (e.key === 'ArrowLeft') go(-1);
      else if (e.key === 'ArrowRight') go(1);
      else if (e.key === '+' || e.key === '=') zoom(1.25);
      else if (e.key === '-') zoom(0.8);
      else if (e.key === '0') setView(null);
      else if (e.key === '1') actual();
      else if (e.key === 'Delete') setConfirming(true);
      else if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'c') void copy();
      else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 's') void save();
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const onDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, from: current };
  };
  const onMove = (e: PointerEvent) => {
    const d = drag.current;
    if (d) setView(clampPan({ ...d.from, x: d.from.x + e.clientX - d.x, y: d.from.y + e.clientY - d.y }, stage, size));
  };
  const onUp = () => { drag.current = null; };

  if (!ready) return <div className="viewer"><p className="empty" role="status">Loading…</p></div>;
  if (!item) return <div className="viewer"><p className="empty" role="alert">{error ?? 'This capture is no longer available.'}</p><button className="btn" onClick={close}>Close</button></div>;

  const percent = Math.round(current.scale * 100);
  return (
    <div className="viewer" role="dialog" aria-modal="true" aria-label={`Capture viewer, ${item.filename}`}>
      <div className="v-main" inert={confirming}>
        <header className="v-head">
          <div className="v-title">
            <strong>{item.filename}</strong>
            <span>{kindLabel(item.kind)} · {formatTime(item.createdAt)} · {item.width} × {item.height}</span>
          </div>
          <span className="v-count" aria-live="polite">{index + 1} of {total}</span>
          <label className="v-folder">
            <span>Folder</span>
            <select value={item.folderId ?? ''} disabled={busy} onChange={e => move(e.target.value || null)}>
              <option value="">None</option>
              {folders.map(f => <option key={f.id} value={f.id}>{f.name}</option>)}
              {item.folderId && !folders.some(f => f.id === item.folderId) && <option value={item.folderId}>Unavailable folder</option>}
            </select>
          </label>
          <button className="icon-btn" onClick={close} aria-label="Close viewer" title="Close (Esc)"><Icon name="close" /></button>
        </header>
        <div className="v-stage" ref={stageEl} tabIndex={0} aria-label="Image canvas: drag to pan, scroll to zoom"
          {...contextMenu.bind(() => [
            { label: 'Copy', onSelect: () => { void copy(); } },
            { label: 'Copy path', onSelect: () => { void run(async () => { await native.copyPath(item.id); setStatus('Path copied'); }); } },
            { label: 'Save as…', onSelect: () => { void save(); } },
            { label: 'Fit to window', onSelect: () => setView(null) },
            { label: 'Actual size', onSelect: actual },
            { label: 'Delete…', danger: true, separatorBefore: true, onSelect: () => setConfirming(true) },
          ])}
          onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={onUp}
          onDoubleClick={() => (view ? setView(null) : actual())} data-panning={view !== null && (current.scale * size.width > stage.width || current.scale * size.height > stage.height)}>
          {src ? (
            <img src={src} alt={`${kindLabel(item.kind)} capture, ${item.width} by ${item.height} pixels`} draggable={false}
              style={{ width: size.width, height: size.height, marginLeft: -size.width / 2, marginTop: -size.height / 2, transform: `translate(${current.x}px, ${current.y}px) scale(${current.scale})` }} />
          ) : <p className="empty" role="status">Loading image…</p>}
          <button className="nav prev" onClick={() => go(-1)} disabled={index === 0} aria-label="Previous capture" title="Previous (←)" onPointerDown={e => e.stopPropagation()}><Icon name="back" /></button>
          <button className="nav next" onClick={() => go(1)} disabled={index >= items.length - 1} aria-label="Next capture" title="Next (→)" onPointerDown={e => e.stopPropagation()}><Icon name="next" /></button>
        </div>
        {error && <p className="field-error v-error" role="alert">{error}</p>}
        <footer className="v-bar">
          <div className="v-zoom" role="group" aria-label="Zoom">
            <button className="icon-btn" onClick={() => zoom(0.8)} aria-label="Zoom out" title="Zoom out (−)"><Icon name="minus" /></button>
            <output aria-label="Zoom level">{percent}%</output>
            <button className="icon-btn" onClick={() => zoom(1.25)} aria-label="Zoom in" title="Zoom in (+)"><Icon name="plus" /></button>
            <button className="btn" onClick={() => setView(null)} aria-pressed={view === null} title="Fit to window (0)"><Icon name="fit" />Fit</button>
            <button className="btn" onClick={actual} title="Actual size (1)">1:1</button>
          </div>
          <span className="status" role="status">{status}</span>
          <div className="v-actions">
            <button className="btn" onClick={copy} disabled={busy} title="Ctrl+C"><Icon name="copy" />Copy</button>
            <button className="btn" onClick={save} disabled={busy} title="Ctrl+Shift+S"><Icon name="save" />Save as</button>
            <button className="btn danger" onClick={() => setConfirming(true)} disabled={busy} title="Delete"><Icon name="trash" />Delete</button>
          </div>
        </footer>
      </div>
      {contextMenu.menu}
      {confirming && (
        <ConfirmDialog hold={false} title="Delete this capture?" confirm="Delete" busy={busy} onCancel={() => setConfirming(false)} onConfirm={remove}
          body="The image and its thumbnail will be permanently removed from Captrain. Files you exported to disk are kept." />
      )}
    </div>
  );
}
