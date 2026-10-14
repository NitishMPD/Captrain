export interface Rect { x: number; y: number; width: number; height: number }
export interface Point { x: number; y: number }
export interface Box { left: number; top: number; width: number; height: number }

const clamp = (v: number, max: number) => Math.min(Math.max(v, 0), max);

/** CSS client coordinates -> physical display pixels, via the element's rect and the overlay's physical size. */
export function toPhysical(clientX: number, clientY: number, box: Box, size: { width: number; height: number }): Point {
  if (box.width <= 0 || box.height <= 0) return { x: 0, y: 0 };
  return {
    x: clamp(((clientX - box.left) * size.width) / box.width, size.width),
    y: clamp(((clientY - box.top) * size.height) / box.height, size.height),
  };
}

/** Normalizes a drag in any direction; null when the rounded selection has no area. */
export function selectionRect(a: Point, b: Point): Rect | null {
  const x = Math.round(Math.min(a.x, b.x));
  const y = Math.round(Math.min(a.y, b.y));
  const width = Math.round(Math.max(a.x, b.x)) - x;
  const height = Math.round(Math.max(a.y, b.y)) - y;
  return width > 0 && height > 0 ? { x, y, width, height } : null;
}

/** First match wins: native supplies windows top-most first (assumption shared with Rust). */
export function windowAt<T extends Rect>(windows: T[], p: Point): T | undefined {
  return windows.find(w => p.x >= w.x && p.x < w.x + w.width && p.y >= w.y && p.y < w.y + w.height);
}
