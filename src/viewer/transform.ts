export interface Size { width: number; height: number }
/** Image centre's offset from the stage centre, in stage pixels, plus the zoom factor. */
export interface Transform { scale: number; x: number; y: number }

export const MIN_SCALE = 0.05;
export const MAX_SCALE = 16;

export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Largest scale that shows the whole image; small images are never upscaled. */
export function fitScale(stage: Size, image: Size): number {
  if (image.width <= 0 || image.height <= 0 || stage.width <= 0 || stage.height <= 0) return 1;
  return Math.min(1, stage.width / image.width, stage.height / image.height);
}

export const fitTransform = (stage: Size, image: Size): Transform => ({ scale: fitScale(stage, image), x: 0, y: 0 });

/** Keeps the image from being dragged away: centred on an axis it fits, edge-bound on one it overflows. */
export function clampPan(t: Transform, stage: Size, image: Size): Transform {
  const limit = (shown: number, view: number) => Math.max(0, (shown - view) / 2);
  const mx = limit(image.width * t.scale, stage.width);
  const my = limit(image.height * t.scale, stage.height);
  return { scale: t.scale, x: clamp(t.x, -mx, mx), y: clamp(t.y, -my, my) };
}

/** Zooms by `factor` keeping the image point under `point` (stage-centre coordinates) fixed. */
export function zoomAt(t: Transform, factor: number, point: { x: number; y: number }, stage: Size, image: Size): Transform {
  const scale = clamp(t.scale * factor, MIN_SCALE, MAX_SCALE);
  const k = scale / t.scale;
  return clampPan({ scale, x: point.x - (point.x - t.x) * k, y: point.y - (point.y - t.y) * k }, stage, image);
}

/** Next index after moving `delta`, staying inside the library. */
export const stepIndex = (index: number, delta: number, total: number) => (total <= 0 ? -1 : clamp(index + delta, 0, total - 1));

/** Index to show after removing the item at `index`; -1 when nothing is left. */
export const indexAfterDelete = (index: number, remaining: number) => (remaining <= 0 ? -1 : Math.min(index, remaining - 1));
