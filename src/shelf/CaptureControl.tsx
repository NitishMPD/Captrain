import { useContextMenu, type MenuItem } from '../ContextMenu';
import { Icon } from '../Icon';
import { native, type CaptureKind, type Folder, type Settings } from '../native';
import { formatShortcut } from '../settings/shortcut';
import { errorMessage, kindLabel } from '../util';

const KINDS: CaptureKind[] = ['region', 'window', 'screen'];

interface Props {
  settings: Settings | null;
  folders: Folder[];
  onPatch: (patch: Partial<Settings>) => void;
  onSettings: (settings: Settings) => void;
  onNewFolder: () => void;
  onRecord: () => void;
  recordingActive: boolean;
  onError: (message: string) => void;
}

/** Bottom dock: split capture button (run / choose type) and the destination folder. */
export function CaptureControl({ settings, folders, onPatch, onSettings, onNewFolder, onRecord, recordingActive, onError }: Props) {
  const captureMenu = useContextMenu();
  const destinationMenu = useContextMenu();
  const kind = settings?.captureKind ?? 'region';
  const active = folders.find(f => f.id === settings?.activeFolder);

  const linkAction: MenuItem = {
    label: 'Link', hint: settings ? formatShortcut(settings.shortcuts.url) : undefined,
    onSelect: () => native.saveLink().catch(e => onError(errorMessage(e))),
  };

  const videoAction: MenuItem = {
    label: 'Video', hint: settings ? formatShortcut(settings.shortcuts.video) : undefined, onSelect: onRecord,
  };

  const capture = (button: HTMLElement) => {
    if (settings?.instantCapture ?? true) {
      native.capture(kind, true).catch(e => onError(errorMessage(e)));
    } else {
      captureMenu.openAbove([...KINDS.map(k => ({ label: kindLabel(k), hint: settings ? formatShortcut(settings.shortcuts[k]) : undefined,
        onSelect: () => native.capture(k, false).catch(e => onError(errorMessage(e))) })), videoAction, linkAction], button);
    }
  };

  const kindMenu = (): MenuItem[] => [
    ...KINDS.map(k => ({ label: kindLabel(k), hint: settings ? formatShortcut(settings.shortcuts[k]) : undefined, checked: k === kind, onSelect: () => onPatch({ captureKind: k }) })),
    videoAction,
    linkAction,
    { label: 'Instant', checked: settings?.instantCapture ?? true, separatorBefore: true, onSelect: () => onPatch({ instantCapture: !(settings?.instantCapture ?? true) }) },
  ];

  const folderMenu = (): MenuItem[] => [
    { label: 'No folder', checked: !active, onSelect: () => onPatch({ activeFolder: null }) },
    ...folders.map(f => ({ label: f.name, checked: f.id === active?.id, onSelect: () => onPatch({ activeFolder: f.id }) })),
    { label: 'New folder…', separatorBefore: true, onSelect: onNewFolder },
    { label: settings?.captureFolder ? 'Change save location…' : 'Choose save location…', separatorBefore: true,
      onSelect: () => native.chooseCaptureFolder().then(onSettings).catch(e => onError(errorMessage(e))) },
    ...(settings?.captureFolder ? [{ label: 'Open save location', onSelect: () => native.openCaptureFolder().catch(e => onError(errorMessage(e))) }] : []),
  ];

  return (
    <footer className="dock">
      <div className="split">
        <button className="split-main" onClick={e => capture(e.currentTarget)} disabled={!settings || recordingActive}
          aria-haspopup={settings?.instantCapture === false ? 'menu' : undefined} aria-expanded={settings?.instantCapture === false ? captureMenu.isOpen : undefined}>
          <Icon name="camera" />
          <span><strong>Capture</strong><small>{kindLabel(kind)}{settings ? ` · ${formatShortcut(settings.shortcuts[kind])}` : ''}</small></span>
        </button>
        <button className="split-arrow" aria-label="Capture options" aria-haspopup="menu" aria-expanded={captureMenu.isOpen} disabled={!settings || recordingActive}
          onClick={e => captureMenu.openAbove(kindMenu(), e.currentTarget)}>
          <Icon name="chevron" />
        </button>
      </div>
      <button className="dest" aria-haspopup="menu" aria-expanded={destinationMenu.isOpen} disabled={!settings}
        onClick={e => destinationMenu.openAbove(folderMenu(), e.currentTarget)}>
        <span><small>Folder</small><strong>{active?.name ?? 'None'}</strong></span>
        <Icon name="folder" />
      </button>
      {captureMenu.menu}
      {destinationMenu.menu}
    </footer>
  );
}
