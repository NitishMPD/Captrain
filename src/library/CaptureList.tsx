import type { CaptureItem } from '../native';
import { formatTime, kindLabel } from '../util';

interface Props {
  items: CaptureItem[];
  thumbs: Record<string, string>;
  freshId?: string | null;
  onOpen: (item: CaptureItem) => void;
}

export function CaptureList({ items, thumbs, freshId, onOpen }: Props) {
  return (
    <ul className="list">
      {items.map(item => (
        <li key={item.id}>
          <button className={`row${item.id === freshId ? ' fresh' : ''}`} onClick={() => onOpen(item)}
            aria-label={`${kindLabel(item.kind)} capture, ${formatTime(item.createdAt)}, ${item.width} by ${item.height} pixels`}>
            <span className="thumb">{thumbs[item.id] && <img src={thumbs[item.id]} alt="" draggable={false} />}</span>
            <span className="meta">
              <strong>{kindLabel(item.kind)}</strong>
              <span>{formatTime(item.createdAt)}</span>
              <span>{item.width} × {item.height}</span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
