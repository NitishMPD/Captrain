import { useEffect, useRef, useState } from 'react';
import { native, type LibraryPage } from '../native';
import { errorMessage } from '../util';
import { loadRange } from './paging';

const EMPTY: LibraryPage = { items: [], total: 0 };

/**
 * Loads one page of metadata, then small thumbnails for that page only.
 * Re-runs on any argument change; superseded requests are ignored and thumbnails
 * for items that left the page are dropped.
 */
export function useCaptures(query: string, offset: number, limit: number, rev: number, onSettled?: () => void, folderId: string | null = null, exportedOnly = false, imagesOnly = false) {
  const [page, setPage] = useState<LibraryPage>(EMPTY);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cache = useRef<Record<string, string>>({});
  const settled = useRef(onSettled);
  settled.current = onSettled;

  useEffect(() => {
    let live = true;
    (async () => {
      try {
        const next = await loadRange((o, l) => native.list(query, o, l, folderId, exportedOnly, imagesOnly), limit, offset);
        if (!live) return;
        const keep: Record<string, string> = {};
        for (const item of next.items) if (cache.current[item.id]) keep[item.id] = cache.current[item.id];
        cache.current = keep;
        setPage(next);
        setThumbs(keep);
        setError(null);
        setLoaded(true);
        await Promise.all(next.items.filter(i => i.kind !== 'url' && !keep[i.id]).map(async i => {
          try {
            const url = await native.image(i.id, true);
            const image = new Image();
            image.src = url;
            await image.decode();
            if (!live) return;
            cache.current[i.id] = url;
            setThumbs({ ...cache.current });
          } catch { /* missing thumbnail keeps the placeholder */ }
        }));
      } catch (e) {
        if (live) setError(errorMessage(e));
      } finally {
        if (live) settled.current?.();
      }
    })();
    return () => { live = false; };
  }, [query, offset, limit, rev, folderId, exportedOnly, imagesOnly]);

  return { ...page, thumbs, loaded, error };
}
