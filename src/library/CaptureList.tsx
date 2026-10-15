import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import type { CaptureItem } from '../native';
import { formatTime, kindLabel, relativeTime } from '../util';

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
            <button className={`row${item.id === freshId ? ' fresh' : ''}`} onClick={() => onOpen(item)} {...bind(() => menuFor(item))}
              aria-haspopup="menu" aria-label={`${item.url ?? item.filename}, ${kindLabel(item.kind)}, ${formatTime(item.createdAt)}, ${item.kind === 'url' ? 'Open in browser' : `${item.width} by ${item.height} pixels`}`}>
              <span className="thumb" data-thumb={item.id}>
                {item.kind === 'url' && <Icon name="link" />}
                {thumbs[item.id] && <img src={thumbs[item.id]} alt="" draggable={false} />}
                <span className="badge"><Icon name={item.exportedPath ? 'save' : item.kind === 'url' ? 'link' : 'image'} /></span>
              </span>
              <span className="meta">
                <strong title={item.url ?? item.filename}>{item.url ?? item.filename}</strong>
                <span>{kindLabel(item.kind)} · {relativeTime(item.createdAt)}</span>
              </span>
            </button>
          </li>
        ))}
      </ul>
      {menu}
    </>
  );
}
