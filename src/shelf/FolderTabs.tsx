import { useEffect, useState } from 'react';
import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import { DRAG_TYPE, type Layout } from '../library/CaptureList';
import { native, type CaptureItem, type Folder } from '../native';

interface Props {
  folders: Folder[];
  selected: string | null;
  /** Folder that receives new captures. */
  active: string | null;
  onSelect: (id: string | null) => void;
  onCreate: () => void;
  /** Cards by default; rows when the list layout is chosen. */
  layout?: Layout;
  emptyLabel?: string;
  onDropItem: (itemId: string, folderId: string) => void;
  menuFor: (folder: Folder) => MenuItem[];
}

/** Icon cards for the folder's newest items, fanned out behind its front. Reloads when the count changes. */
function FolderPeek({ folder }: { folder: Folder }) {
  const [kinds, setKinds] = useState<{ id: string; kind: CaptureItem['kind'] }[]>([]);
  useEffect(() => {
    let live = true;
    if (folder.count === 0) { setKinds([]); return; }
    native.list('', 0, 3, folder.id)
      .then(page => { if (live) setKinds(page.items.map(({ id, kind }) => ({ id, kind }))); })
      .catch(() => { /* decoration only; the count still shows */ });
    return () => { live = false; };
  }, [folder.id, folder.count]);
  if (kinds.length === 0) return null;
  return (
    <span className="folder-peek" aria-hidden="true">
      {kinds.map(({ id, kind }) => (
        <span key={id} className="peek" data-kind={kind}><Icon name={kind === 'video' ? 'video' : kind === 'url' ? 'link' : 'image'} /></span>
      ))}
    </span>
  );
}

export function FolderTabs({ folders, selected, active, onSelect, onCreate, layout = 'grid', emptyLabel, onDropItem, menuFor }: Props) {
  const { menu, bind } = useContextMenu();
  if (folders.length === 0) {
    return <button className="folder-empty" onClick={onCreate}><Icon name="folderPlus" />{emptyLabel ?? 'Create a folder to organize captures'}</button>;
  }
  return (
    <>
      <ul className={layout === 'list' ? 'folder-rows' : 'folders'} aria-label="Folders">
        {folders.map((f, i) => (
          <li key={f.id}>
            <button className={layout === 'list' ? 'row folder-row' : 'folder'} data-permanent={f.path !== null} data-hue={i % 5} aria-pressed={f.id === selected}
              onDragOver={e => { if (e.dataTransfer.types.includes(DRAG_TYPE)) { e.preventDefault(); e.currentTarget.dataset.over = 'true'; } }}
              onDragLeave={e => { delete e.currentTarget.dataset.over; }}
              onDrop={e => { delete e.currentTarget.dataset.over; const id = e.dataTransfer.getData(DRAG_TYPE); if (id) { e.preventDefault(); onDropItem(id, f.id); } }} onClick={() => onSelect(f.id === selected ? null : f.id)}
              {...bind(() => menuFor(f))} aria-haspopup="menu">
              {layout !== 'list' && <FolderPeek folder={f} />}
              <span className="folder-icon"><Icon name={f.path ? 'folderPc' : 'folder'} /></span>
              {layout === 'list'
                ? <span className="meta"><strong title={f.path ?? f.name}>{f.name}</strong>{f.path && <span className="link-address" title={f.path}>{f.path}</span>}<span>{f.count === 1 ? '1 file' : `${f.count} files`}{f.id === active ? ' · default' : ''}</span></span>
                : <><strong>{f.name}</strong><span>{f.count === 1 ? '1 file' : `${f.count} files`}{f.path ? ' · on PC' : ''}{f.id === active ? ' · default' : ''}</span></>}
            </button>
          </li>
        ))}
      </ul>
      {menu}
    </>
  );
}
