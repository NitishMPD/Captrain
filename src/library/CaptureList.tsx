import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import type { CaptureItem } from '../native';
import { formatTime, kindLabel, relativeTime } from '../util';

export function linkAddress(value: string | null): string {
  if (!value) return '';
  try {
    const url = new URL(value);
    if (url.protocol !== 'file:') return value;
    const path = decodeURIComponent(url.pathname).replace(/^\/([a-z]:)/i, '$1').replaceAll('/', '\\');
    return url.hostname ? `\\\\${url.hostname}${path}` : path;
  } catch { return value; }
}

export function linkTitle(item: CaptureItem): string {
  if (item.title) return item.title;
  if (item.url?.startsWith('file:')) return linkAddress(item.url).replace(/\\+$/, '').split('\\').at(-1) || item.filename;
  try { return new URL(item.url ?? '').host; } catch { return item.url ?? item.filename; }
}

export const linkOpenLabel = (item: CaptureItem) => item.url?.startsWith('file:') ? 'Open in Explorer' : 'Open in browser';

export type Layout = 'list' | 'grid';

interface Props {
  items: CaptureItem[];
  thumbs: Record<string, string>;
  layout: Layout;
  freshId?: string | null;
  onOpen: (item: CaptureItem) => void;
  menuFor: (item: CaptureItem) => MenuItem[];
  selection?: ReadonlySet<string> | null;
  disabled?: boolean;
}

/** Drag payload type: a capture id dropped onto a folder tile. */
export const DRAG_TYPE = 'application/x-captrain-capture';

/** A compact card (thumbnail + title) follows the pointer instead of the whole row. */
function dragPreview(transfer: DataTransfer, row: HTMLElement) {
  const ghost = document.createElement('div');
  ghost.className = 'drag-ghost';
  const thumb = row.querySelector('.thumb');
  if (thumb) ghost.append(thumb.cloneNode(true));
  const title = document.createElement('strong');
  title.textContent = row.querySelector('.meta strong')?.textContent ?? '';
  ghost.append(title);
  document.body.append(ghost);
  transfer.setDragImage(ghost, 20, 20);
  // The browser snapshots the element synchronously; it only has to exist for this frame.
  requestAnimationFrame(() => ghost.remove());
}

export function CaptureList({ items, thumbs, layout, freshId, onOpen, menuFor, selection, disabled }: Props) {
  const { menu, bind } = useContextMenu();
  return (
    <>
      <ul className={layout === 'grid' ? 'grid' : 'list'}>
        {items.map(item => (
          <li key={item.id}>
            <button disabled={disabled} role={selection ? 'checkbox' : undefined} aria-checked={selection ? selection.has(item.id) : undefined} draggable={!selection} onDragStart={e => { e.dataTransfer.setData(DRAG_TYPE, item.id); e.dataTransfer.effectAllowed = 'move'; dragPreview(e.dataTransfer, e.currentTarget); }} className={`row${item.id === freshId ? ' fresh' : ''}${item.kind === 'url' ? ' link-row' : ''}`} onClick={() => onOpen(item)} {...bind(() => menuFor(item))}
              aria-haspopup="menu" aria-label={`${item.kind === 'url' ? `${linkTitle(item)}, ${linkAddress(item.url)}` : item.filename}, ${kindLabel(item.kind)}, ${formatTime(item.createdAt)}, ${selection ? 'Toggle selection' : item.kind === 'url' ? linkOpenLabel(item) : `${item.width} by ${item.height} pixels`}`}>
              {selection && <span className="selection-check" aria-hidden="true">{selection.has(item.id) && <Icon name="check" />}</span>}
              <span className="thumb" data-thumb={item.id}>
                {(item.kind === 'url' || item.kind === 'video') && <Icon name={item.kind === 'video' ? 'video' : item.url?.startsWith('file:') ? 'folder' : 'link'} />}
                {thumbs[item.id] && <img src={thumbs[item.id]} alt="" draggable={false} />}
                {((item.kind !== 'url' && item.kind !== 'video') || item.exportedPath) && <span className="badge"><Icon name={item.exportedPath ? 'save' : 'image'} /></span>}
              </span>
              <span className="meta">
                <strong title={item.url ?? item.filename}>{item.kind === 'url' ? linkTitle(item) : item.filename}</strong>
                {item.kind === 'url' && item.url && <span className="link-address" title={linkAddress(item.url)}>{linkAddress(item.url)}</span>}
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
