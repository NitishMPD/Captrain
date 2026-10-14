import { useEffect, useState } from 'react';
import { Icon } from '../Icon';
import type { CaptureItem } from '../native';
import { CaptureList } from './CaptureList';
import { useCaptures } from './useCaptures';

const PAGE = 20;

interface Props { rev: number; onBack: () => void; onOpen: (item: CaptureItem) => void }

export function Library({ rev, onBack, onOpen }: Props) {
  const [input, setInput] = useState('');
  const [query, setQuery] = useState('');
  const [offset, setOffset] = useState(0);
  const { items, total, thumbs, loaded, error } = useCaptures(query, offset, PAGE, rev);

  useEffect(() => {
    const t = setTimeout(() => { setQuery(input.trim()); setOffset(0); }, 200);
    return () => clearTimeout(t);
  }, [input]);

  // After a delete the current page can vanish; step back to the last real page.
  useEffect(() => {
    if (loaded && items.length === 0 && total > 0 && offset > 0) setOffset(Math.floor((total - 1) / PAGE) * PAGE);
  }, [loaded, items.length, total, offset]);

  return (
    <>
      <header className="header">
        <button className="icon-btn" onClick={onBack} aria-label="Back to recent captures"><Icon name="back" /></button>
        <input className="search" type="search" autoFocus value={input} onChange={e => setInput(e.target.value)}
          placeholder="Search kind, date or filename" aria-label="Search captures" />
      </header>
      <main className="body">
        {error ? <p className="empty">{error}</p>
          : !loaded ? <p className="empty">Loading…</p>
          : items.length === 0 ? <p className="empty">{query ? `No captures match “${query}”.` : 'No captures yet.'}</p>
          : <CaptureList items={items} thumbs={thumbs} onOpen={onOpen} />}
      </main>
      {total > PAGE && (
        <nav className="pager" aria-label="Library pages">
          <button className="btn" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE))}>Previous</button>
          <span aria-live="polite">{offset + 1}–{Math.min(offset + PAGE, total)} of {total}</span>
          <button className="btn" disabled={offset + PAGE >= total} onClick={() => setOffset(offset + PAGE)}>Next</button>
        </nav>
      )}
    </>
  );
}
