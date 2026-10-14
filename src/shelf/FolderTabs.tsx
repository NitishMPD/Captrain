import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import type { Folder } from '../native';

interface Props {
  folders: Folder[];
  selected: string | null;
  /** Folder that receives new captures. */
  active: string | null;
  onSelect: (id: string | null) => void;
  onCreate: () => void;
  menuFor: (folder: Folder) => MenuItem[];
}

export function FolderTabs({ folders, selected, active, onSelect, onCreate, menuFor }: Props) {
  const { menu, bind } = useContextMenu();
  if (folders.length === 0) {
    return <button className="folder-empty" onClick={onCreate}><Icon name="folderPlus" />Create a folder to organize captures</button>;
  }
  return (
    <>
      <ul className="folders" aria-label="Folders">
        {folders.map((f, i) => (
          <li key={f.id}>
            <button className="folder" data-hue={i % 5} data-empty={f.count === 0} aria-pressed={f.id === selected} onClick={() => onSelect(f.id === selected ? null : f.id)}
              {...bind(() => menuFor(f))} aria-haspopup="menu">
              <span className="folder-icon"><Icon name="folder" /></span>
              <strong>{f.name}</strong>
              <span>{f.count === 1 ? '1 file' : `${f.count} files`}{f.id === active ? ' · saving here' : ''}</span>
            </button>
          </li>
        ))}
      </ul>
      {menu}
    </>
  );
}
