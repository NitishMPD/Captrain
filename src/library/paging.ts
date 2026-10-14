import type { CaptureItem, LibraryPage } from '../native';

/** Native clamps `limit` to this; larger requests must be split. */
export const MAX_PAGE = 100;

export type FetchPage = (offset: number, limit: number) => Promise<LibraryPage>;

/** Loads `count` items starting at `start` as consecutive pages of at most MAX_PAGE, never dropping earlier ones. */
export async function loadRange(fetch: FetchPage, count: number, start = 0): Promise<LibraryPage> {
  const items: CaptureItem[] = [];
  let total = 0;
  while (items.length < count && (items.length === 0 || start + items.length < total)) {
    const page = await fetch(start + items.length, Math.min(MAX_PAGE, count - items.length));
    total = page.total;
    if (page.items.length === 0) break;
    items.push(...page.items);
  }
  return { items, total };
}

/** Reads a context from the top until `id` is loaded; `index` is -1 when it is no longer in the context. */
export async function locate(fetch: FetchPage, id: string): Promise<LibraryPage & { index: number }> {
  const items: CaptureItem[] = [];
  let total = 0;
  for (;;) {
    const page = await fetch(items.length, MAX_PAGE);
    total = page.total;
    items.push(...page.items);
    const index = items.findIndex(i => i.id === id);
    if (index >= 0 || page.items.length === 0 || items.length >= total) return { items, total, index };
  }
}
