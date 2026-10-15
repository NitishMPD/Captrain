import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import type { CaptureItem } from '../native';
import { formatTime, kindLabel, relativeTime } from '../util';

function linkTitle(url: string | null, fallback: string) {
  try { return new URL(url ?? '').host; } catch { return url ?? fallback; }
}

export type Layout = 'list' | 'grid';

interface Props {
  items: CaptureItem[];
  thumbs: Record<string, string>;
  layout: Layout;
  freshId?: string | null;
  onOpen: (item: CaptureItem) => void;
  menuFor: (item: CaptureItem) => MenuItem[];
}

export function CaptureList({ items, thumbs, layout, freshId, onOpen, menuFor }: Props) {
  const { menu, bind } = useContextMenu();
  return (
    <>
      <ul className={layout === 'grid' ? 'grid' : 'list'}>
        {items.map(item => (
          <li key={item.id}>
            <button className={`row${item.id === freshId ? ' fresh' : ''}${item.kind === 'url' ? ' link-row' : ''}`} onClick={() => onOpen(item)} {...bind(() => menuFor(item))}
              aria-haspopup="menu" aria-label={`${item.url ?? item.filename}, ${kindLabel(item.kind)}, ${formatTime(item.createdAt)}, ${item.kind === 'url' ? 'Open in browser' : `${item.width} by ${item.height} pixels`}`}>
              <span className="thumb" data-thumb={item.id}>
                {(item.kind === 'url' || item.kind === 'video') && <Icon name={item.kind === 'video' ? 'video' : 'link'} />}
                {thumbs[item.id] && <img src={thumbs[item.id]} alt="" draggable={false} />}
                {((item.kind !== 'url' && item.kind !== 'video') || item.exportedPath) && <span className="badge"><Icon name={item.exportedPath ? 'save' : 'image'} /></span>}
              </span>
              <span className="meta">
                <strong title={item.url ?? item.filename}>{item.kind === 'url' ? linkTitle(item.url, item.filename) : item.filename}</strong>
                {item.kind === 'url' && item.url && <span className="link-address" title={item.url}>{item.url}</span>}
                <span>{kindLabel(item.kind)} · {relativeTime(item.createdAt)}</span>
              </span>
              {item.kind === 'url' && <span className="link-open"><Icon name="external" /></span>}
            </button>
          </li>
        ))}
      </ul>
      {menu}
    </>
  );
}
