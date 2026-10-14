import { describe, expect, it } from 'vitest';
import { selectionRect, toPhysical, windowAt } from './geometry';

describe('overlay geometry', () => {
  it('scales CSS pixels to physical pixels using the element rect', () => {
    const box = { left: 10, top: 20, width: 1280, height: 720 };
    expect(toPhysical(10 + 640, 20 + 360, box, { width: 1920, height: 1080 })).toEqual({ x: 960, y: 540 });
  });

  it('clamps pointer positions outside the element', () => {
    const box = { left: 0, top: 0, width: 100, height: 100 };
    expect(toPhysical(-5, 500, box, { width: 200, height: 200 })).toEqual({ x: 0, y: 200 });
  });

  it('normalizes reverse drags', () => {
    expect(selectionRect({ x: 50, y: 80 }, { x: 10, y: 20 })).toEqual({ x: 10, y: 20, width: 40, height: 60 });
  });

  it('rejects zero-area selections', () => {
    expect(selectionRect({ x: 5, y: 5 }, { x: 5, y: 90 })).toBeNull();
    expect(selectionRect({ x: 5.2, y: 5.2 }, { x: 5.4, y: 5.4 })).toBeNull();
  });

  it('finds the first window containing the point', () => {
    const windows = [
      { id: 'a', x: 0, y: 0, width: 100, height: 100 },
      { id: 'b', x: 0, y: 0, width: 500, height: 500 },
    ];
    expect(windowAt(windows, { x: 50, y: 50 })?.id).toBe('a');
    expect(windowAt(windows, { x: 100, y: 100 })?.id).toBe('b');
    expect(windowAt(windows, { x: 600, y: 10 })).toBeUndefined();
  });
});
