import { useEffect, useRef, useState, type PointerEvent } from 'react';
import { native, type OverlayData } from '../native';
import { errorMessage, quiet } from '../util';
import { selectionRect, toPhysical, windowAt, type Point, type Rect } from './geometry';

const pct = (v: number, total: number) => `${(v / total) * 100}%`;
const place = (r: Rect, d: OverlayData) => ({ left: pct(r.x, d.width), top: pct(r.y, d.height), width: pct(r.width, d.width), height: pct(r.height, d.height) });

export function Overlay() {
  const [data, setData] = useState<OverlayData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ a: Point; b: Point } | null>(null);
  const [hoverId, setHoverId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const stage = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    native.overlay().then(d => { if (live) setData(d); }).catch(e => { if (live) setError(errorMessage(e)); });
    return () => { live = false; };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); quiet(native.cancel()); } };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const point = (e: PointerEvent): Point =>
    toPhysical(e.clientX, e.clientY, stage.current!.getBoundingClientRect(), data!);

  const submit = async (selection: Parameters<typeof native.select>[0]) => {
    setBusy(true);
    setError(null);
    try { await native.select(selection); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  };

  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!data || busy || e.button !== 0 || data.kind !== 'region') return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setError(null);
    const p = point(e);
    setDrag({ a: p, b: p });
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!data || busy) return;
    const p = point(e);
    if (data.kind === 'region') setDrag(d => d && { ...d, b: p });
    else setHoverId(windowAt(data.windows, p)?.id ?? null);
  };
  const onUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!data || busy || e.button !== 0) return;
    const p = point(e);
    if (data.kind === 'region') {
      if (!drag) return;
      const rect = selectionRect(drag.a, p);
      setDrag(null);
      if (rect) void submit(rect);
      else setError('The selection is empty. Drag across the area you want to capture.');
    } else {
      const target = windowAt(data.windows, p);
      if (target) void submit({ windowId: target.id });
      else setError('No capturable window here. Move over a window and click it.');
    }
  };

  const selection = data && drag ? selectionRect(drag.a, drag.b) : null;
  const hovered = data?.kind === 'window' ? data.windows.find(w => w.id === hoverId) : undefined;
  const cutout = selection ?? hovered;

  return (
    <div className="ov">
      <div ref={stage} className="ov-stage" onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp}
        onPointerCancel={() => setDrag(null)} onPointerLeave={() => setHoverId(null)}>
        {data?.image && <img src={data.image} alt="" draggable={false} />}
        {data && !cutout && <div className="ov-dim" />}
        {data && cutout && (
          <div className={`ov-cut${selection ? ' region' : ''}`} style={place(cutout, data)}>
            {selection && <span className="ov-size">{selection.width} × {selection.height}</span>}
          </div>
        )}
      </div>
      <div className="ov-bar" role="group" aria-label="Capture selection">
        <span role={error ? 'alert' : 'status'} className={error ? 'ov-error' : undefined}>
          {error ?? (!data ? 'Preparing capture…' : busy ? 'Capturing…' : data.kind === 'region' ? 'Drag to select a region' : 'Click a window to capture it')}
        </span>
        <button className="btn" onClick={() => quiet(native.cancel())}>Cancel <kbd>Esc</kbd></button>
      </div>
    </div>
  );
}
