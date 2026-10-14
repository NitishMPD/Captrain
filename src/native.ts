import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type CaptureKind = 'region' | 'window' | 'screen';
export type Mode = 'collapsed' | 'temporary' | 'manual' | 'capturing';
export interface CaptureItem { id: string; kind: CaptureKind; createdAt: string; width: number; height: number; filename: string; folderId: string | null; exportedPath: string | null; }
export interface Folder { id: string; name: string; count: number; }
export interface ViewerContext { id: string; query: string; folderId: string | null; exportedOnly: boolean; }
export interface ShelfState { mode: Mode; hovered: boolean; error: string | null; reveal: number; }
export interface Settings { edge: 'left' | 'right'; display: string | null; shortcuts: Record<'region' | 'window' | 'screen' | 'toggle', string>; captureFolder: string | null; instantCapture: boolean; captureKind: CaptureKind; activeFolder: string | null; }
export interface Display { id: string; name: string; }
export interface LibraryPage { items: CaptureItem[]; total: number; }
export interface OverlayData { kind: 'region' | 'window'; image: string | null; width: number; height: number; windows: { id: string; x: number; y: number; width: number; height: number }[]; }

export const native = {
  state: () => invoke<ShelfState>('shelf_state'),
  shelf: (action: 'open' | 'collapse' | 'toggle' | 'enter' | 'leave' | 'interact' | 'hold' | 'release' | 'ready' | 'clearError', reveal?: number) => invoke<void>('shelf_action', { action, reveal }),
  capture: (kind: CaptureKind, direct = false) => invoke<void>('start_capture', { kind, direct }),
  list: (query = '', offset = 0, limit = 20, folderId: string | null = null, exportedOnly = false) => invoke<LibraryPage>('list_captures', { query, offset, limit, folderId, exportedOnly }),
  folders: () => invoke<Folder[]>('list_folders'),
  createFolder: (name: string) => invoke<Folder>('create_folder', { name }),
  renameFolder: (id: string, name: string) => invoke<void>('rename_folder', { id, name }),
  deleteFolder: (id: string) => invoke<void>('delete_folder', { id }),
  moveCapture: (id: string, folderId: string | null) => invoke<void>('move_capture', { id, folderId }),
  chooseCaptureFolder: () => invoke<Settings>('choose_capture_folder'),
  openCaptureFolder: () => invoke<void>('open_capture_folder'),
  storagePath: () => invoke<string>('storage_path'),
  openViewer: (id: string, query = '', folderId: string | null = null, exportedOnly = false) => invoke<void>('open_viewer', { id, query, folderId, exportedOnly }),
  viewerContext: () => invoke<ViewerContext>('viewer_context'),
  closeViewer: () => invoke<void>('close_viewer'),
  image: (id: string, thumbnail = false) => invoke<string>('capture_image', { id, thumbnail }),
  copy: (id: string) => invoke<void>('copy_capture', { id }),
  save: (id: string) => invoke<boolean>('save_capture', { id }),
  delete: (id: string) => invoke<void>('delete_capture', { id }),
  settings: () => invoke<Settings>('get_settings'),
  displays: () => invoke<Display[]>('list_displays'),
  updateSettings: (settings: Settings) => invoke<Settings>('update_settings', { settings }),
  defaults: () => invoke<Settings>('restore_shortcuts'),
  recording: (active: boolean) => invoke<void>('record_shortcuts', { active }),
  onRecorded: (fn: (binding: string) => void) => listen<string>('shortcut-recorded', e => fn(e.payload)),
  overlay: () => invoke<OverlayData>('overlay_data'),
  select: (selection: { x?: number; y?: number; width?: number; height?: number; windowId?: string }) => invoke<void>('finish_selection', { selection }),
  cancel: () => invoke<void>('cancel_capture'),
  onState: (fn: (state: ShelfState) => void) => listen<ShelfState>('shelf-state', e => fn(e.payload)),
  onCapture: (fn: (item: CaptureItem) => void) => listen<CaptureItem>('capture-created', e => fn(e.payload)),
  onLibraryChanged: (fn: () => void) => listen('library-changed', fn),
};
