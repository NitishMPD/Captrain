import type { BatchResult, CaptureItem } from '../native';
import { errorMessage } from '../util';

/** Each native mutation commits independently; failed items remain available for retry. */
export async function applyToSelection(items: CaptureItem[], action: (id: string) => Promise<void>): Promise<BatchResult> {
  const result: BatchResult = { completed: [], errors: [] };
  for (const item of items) {
    try { await action(item.id); result.completed.push(item.id); }
    catch (e) { result.errors.push(`${item.title ?? item.filename}: ${errorMessage(e)}`); }
  }
  return result;
}
