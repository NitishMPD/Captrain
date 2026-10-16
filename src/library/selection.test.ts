import { describe, expect, it } from 'vitest';
import type { CaptureItem } from '../native';
import { applyToSelection } from './selection';

describe('batch selection actions', () => {
  it('keeps successful and failed items separate and continues after failures', async () => {
    const items = ['first', 'failed', 'last'].map(id => ({ id, filename: `${id}.png`, title: null } as CaptureItem));
    const calls: string[] = [];
    const result = await applyToSelection(items, async id => {
      calls.push(id);
      if (id === 'failed') throw new Error('Disk unavailable');
    });
    expect(calls).toEqual(['first', 'failed', 'last']);
    expect(result.completed).toEqual(['first', 'last']);
    expect(result.errors).toEqual(['failed.png: Disk unavailable']);
  });
});
