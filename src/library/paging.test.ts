import { describe, expect, it } from 'vitest';
import type { CaptureItem } from '../native';
import { loadRange, locate, MAX_PAGE, type FetchPage } from './paging';

const make = (n: number): CaptureItem[] => Array.from({ length: n }, (_, i) => ({
  id: `c${i}`, kind: 'region', createdAt: '2026-01-01T00:00:00Z', width: 1, height: 1, filename: `c${i}.png`, folderId: null, exportedPath: null,
}));

/** Mimics native: limit is clamped to 100. */
const source = (all: CaptureItem[], calls: [number, number][] = []): FetchPage => async (offset, limit) => {
  calls.push([offset, limit]);
  return { items: all.slice(offset, offset + Math.min(limit, 100)), total: all.length };
};

describe('paging', () => {
  it('accumulates more than one native page without dropping older ones', async () => {
    const calls: [number, number][] = [];
    const page = await loadRange(source(make(250), calls), 240);
    expect(page.items).toHaveLength(240);
    expect(page.items.map(i => i.id).slice(95, 105)).toEqual(['c95', 'c96', 'c97', 'c98', 'c99', 'c100', 'c101', 'c102', 'c103', 'c104']);
    expect(page.total).toBe(250);
    expect(calls).toEqual([[0, MAX_PAGE], [100, MAX_PAGE], [200, 40]]);
  });

  it('stops at the end of the library', async () => {
    const page = await loadRange(source(make(130)), 500);
    expect(page.items).toHaveLength(130);
  });

  it('returns an empty library', async () => {
    expect(await loadRange(source([]), 20)).toEqual({ items: [], total: 0 });
  });

  it('finds an item deep in the active context', async () => {
    const found = await locate(source(make(350)), 'c273');
    expect(found.index).toBe(273);
    expect(found.items).toHaveLength(300); // stops once the page holding it is loaded
    expect(found.total).toBe(350);
  });

  it('reports an item that left the context', async () => {
    expect((await locate(source(make(120)), 'gone')).index).toBe(-1);
  });
});
