import { describe, expect, it } from 'vitest';
import { clampPan, fitScale, indexAfterDelete, MAX_SCALE, stepIndex, zoomAt } from './transform';

const stage = { width: 1000, height: 600 };
const image = { width: 2000, height: 1000 };

describe('viewer transform', () => {
  it('fits large images and never upscales small ones', () => {
    expect(fitScale(stage, image)).toBe(0.5);
    expect(fitScale(stage, { width: 100, height: 50 })).toBe(1);
    expect(fitScale(stage, { width: 0, height: 0 })).toBe(1);
  });

  it('centres an axis that fits and bounds one that overflows', () => {
    const t = clampPan({ scale: 1, x: 900, y: 900 }, stage, image);
    expect(t.x).toBe(500); // (2000 - 1000) / 2
    expect(t.y).toBe(200); // (1000 - 600) / 2
    expect(clampPan({ scale: 0.25, x: 80, y: 80 }, stage, image)).toEqual({ scale: 0.25, x: 0, y: 0 });
  });

  it('zooms around the pointer so its image point stays put', () => {
    const start = { scale: 1, x: 0, y: 0 };
    const point = { x: 200, y: 100 };
    const next = zoomAt(start, 2, point, stage, image);
    expect(next.scale).toBe(2);
    // image point under the pointer before: (point - t) / scale; after must match
    expect((point.x - next.x) / next.scale).toBeCloseTo((point.x - start.x) / start.scale);
    expect((point.y - next.y) / next.scale).toBeCloseTo((point.y - start.y) / start.scale);
  });

  it('limits zoom', () => {
    expect(zoomAt({ scale: MAX_SCALE, x: 0, y: 0 }, 2, { x: 0, y: 0 }, stage, image).scale).toBe(MAX_SCALE);
  });

  it('steps without leaving the library', () => {
    expect(stepIndex(0, -1, 5)).toBe(0);
    expect(stepIndex(4, 1, 5)).toBe(4);
    expect(stepIndex(2, 1, 5)).toBe(3);
    expect(stepIndex(0, 1, 0)).toBe(-1);
  });

  it('picks the neighbour after a delete', () => {
    expect(indexAfterDelete(2, 4)).toBe(2); // the next item slid into place
    expect(indexAfterDelete(3, 3)).toBe(2); // deleted the last one
    expect(indexAfterDelete(0, 0)).toBe(-1);
  });
});
