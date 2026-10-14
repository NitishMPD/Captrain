import { useEffect, useRef, useState } from 'react';
import { native, type LibraryPage } from '../native';
import { errorMessage } from '../util';

const EMPTY: LibraryPage = { items: [], total: 0 };

/**
 * Loads one page of metadata, then small thumbnails for that page only.
 * Re-runs on any argument change; superseded requests are ignored and thumbnails
 * for items that left the page are dropped.
 */
export function useCaptures(query: string, offset: number, limit: number, rev: number, onSettled?: () => void) {
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
        const next = await native.list(query, offset, limit);
        if (!live) return;
        const keep: Record<string, string> = {};
        for (const item of next.items) if (cache.current[item.id]) keep[item.id] = cache.current[item.id];
        cache.current = keep;
        setPage(next);
        setThumbs(keep);
        setError(null);
        setLoaded(true);
        await Promise.all(next.items.filter(i => !keep[i.id]).map(async i => {
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
  }, [query, offset, limit, rev]);

  return { ...page, thumbs, loaded, error };
}
