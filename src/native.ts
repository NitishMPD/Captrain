import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type CaptureKind = 'region' | 'window' | 'screen';
export type Mode = 'collapsed' | 'temporary' | 'manual' | 'capturing';
export interface CaptureItem { id: string; kind: CaptureKind | 'url' | 'video'; url: string | null; title: string | null; createdAt: string; width: number; height: number; filename: string; folderId: string | null; exportedPath: string | null; }
/** `path` is set for a permanent folder: captures filed here are also saved into that PC directory. */
export interface Folder { id: string; name: string; path: string | null; count: number; }
export interface Destinations { image: string | null; region: string | null; link: string | null; video: string | null; }
export interface ViewerContext { id: string; query: string; folderId: string | null; exportedOnly: boolean; }
export interface ShelfState { mode: Mode; hovered: boolean; error: string | null; reveal: number; }
export interface Settings { edge: 'left' | 'right'; display: string | null; edgePosition: number; shortcutsOnly: boolean; morphAnimation: boolean; shortcuts: Record<'region' | 'window' | 'screen' | 'url' | 'video' | 'toggle', string>; captureFolder: string | null; instantCapture: boolean; captureKind: CaptureKind; activeFolder: string | null; destinations: Destinations; theme: 'system' | 'light' | 'dark'; }
export interface Display { id: string; name: string; }
export interface LibraryPage { items: CaptureItem[]; total: number; }
export interface BatchResult { completed: string[]; errors: string[]; }
export interface CloudOptions { images: boolean; links: boolean; videos: boolean; includeExisting: boolean; paused: boolean; }
export interface CloudStatus { configured: boolean; connected: boolean; account: string | null; options: CloudOptions; lastBackup: string | null; busy: boolean; phase: string; error: string | null; }
export interface OverlayData { kind: 'region' | 'window'; image: string | null; width: number; height: number; windows: { id: string; x: number; y: number; width: number; height: number }[]; }

export interface RecordingOptions {
  kind: CaptureKind; display: string; window: string | null;
  fps: 30 | 60; quality: 'standard' | 'high' | 'maximum'; resolution: 0 | 720 | 1080;
  cursor: boolean; countdown: boolean; systemAudio: boolean;
  microphone: string | null; camera: string | null;
  corner: 'TopLeft' | 'TopRight' | 'BottomLeft' | 'BottomRight'; cameraSize: 15 | 22 | 30;
}
export interface RecordingState { status: string; elapsedMs: number; frames: number; error: string | null; options: RecordingOptions | null; }
export interface RecordingDevice { id: string; name: string; }
export interface RecordingDevices { microphones: RecordingDevice[]; cameras: RecordingDevice[]; }
export interface RecordingTargets { displays: RecordingDevice[]; windows: RecordingDevice[]; }

export const native = {
  cloudStatus: () => invoke<CloudStatus>('cloud_status'),
  cloudConfigure: () => invoke<CloudStatus>('cloud_configure'),
  cloudOptions: (options: CloudOptions) => invoke<CloudStatus>('cloud_options', { options }),
  cloudAction: (action: 'connect' | 'backup' | 'restore' | 'disconnect' | 'cancel') => invoke<CloudStatus>('cloud_action', { action }),
  recordingPreferences: (options?: RecordingOptions) => invoke<RecordingOptions | null>('recording_preferences', { options: options ?? null }),
  countdownReady: () => invoke<void>('countdown_ready'),
  recordingTargets: () => invoke<RecordingTargets>('recording_targets'),
  recordingDevices: () => invoke<RecordingDevices>('recording_devices'),
  recordingState: () => invoke<RecordingState>('recording_state'),
  startRecording: (options: RecordingOptions) => invoke<void>('start_recording', { options }),
  controlRecording: (action: 'stop' | 'pause' | 'resume') => invoke<void>('recording_control', { action }),
  playRecording: (id: string) => invoke<void>('play_recording', { id }),
  onOpenRecording: (fn: () => void) => listen('open-recording', fn),
  onRecordingState: (fn: (state: RecordingState) => void) => listen<RecordingState>('recording-state', e => fn(e.payload)),
  dragShelf: () => invoke<boolean>('drag_shelf'),
  state: () => invoke<ShelfState>('shelf_state'),
  shelf: (action: 'open' | 'collapse' | 'toggle' | 'enter' | 'leave' | 'interact' | 'hold' | 'release' | 'ready' | 'clearError', reveal?: number) => invoke<void>('shelf_action', { action, reveal }),
  capture: (kind: CaptureKind, direct = false) => invoke<void>('start_capture', { kind, direct }),
  saveLink: () => invoke<void>('save_link'),
  renameLink: (id: string, title: string) => invoke<void>('rename_link', { id, title }),
  openLink: (id: string) => invoke<void>('open_link', { id }),
  startup: () => invoke<boolean>('get_startup'),
  setStartup: (enabled: boolean) => invoke<boolean>('set_startup', { enabled }),
  onSettingsChanged: (fn: (settings: Settings) => void) => listen<Settings>('settings-changed', e => fn(e.payload)),
  list: (query = '', offset = 0, limit = 20, folderId: string | null = null, exportedOnly = false, imagesOnly = false, capturesOnly = false) => invoke<LibraryPage>('list_captures', { query, offset, limit, folderId, exportedOnly, imagesOnly, capturesOnly }),
  folders: () => invoke<Folder[]>('list_folders'),
  createPermanentFolder: () => invoke<Folder | null>('create_permanent_folder'),
  createFolder: (name: string) => invoke<Folder>('create_folder', { name }),
  renameFolder: (id: string, name: string) => invoke<void>('rename_folder', { id, name }),
  deleteFolder: (id: string) => invoke<void>('delete_folder', { id }),
  moveCapture: (id: string, folderId: string | null) => invoke<void>('move_capture', { id, folderId }),
  chooseCaptureFolder: () => invoke<Settings>('choose_capture_folder'),
  openCaptureFolder: () => invoke<void>('open_capture_folder'),
  reveal: (id: string) => invoke<void>('reveal_capture', { id }),
  openFolder: (id: string) => invoke<void>('open_folder', { id }),
  storagePath: () => invoke<string>('storage_path'),
  openViewer: (id: string, query = '', folderId: string | null = null, exportedOnly = false) => invoke<void>('open_viewer', { id, query, folderId, exportedOnly }),
  viewerContext: () => invoke<ViewerContext>('viewer_context'),
  closeViewer: () => invoke<void>('close_viewer'),
  image: (id: string, thumbnail = false) => invoke<string>('capture_image', { id, thumbnail }),
  copyPath: (id: string, folder = false) => invoke<void>('copy_path', { id, folder }),
  copy: (id: string) => invoke<void>('copy_capture', { id }),
  save: (id: string) => invoke<boolean>('save_capture', { id }),
  saveSelected: (ids: string[]) => invoke<BatchResult | null>('save_selected', { ids }),
  delete: (id: string) => invoke<void>('delete_capture', { id }),
  settings: () => invoke<Settings>('get_settings'),
  displays: () => invoke<Display[]>('list_displays'),
  updateSettings: (settings: Settings) => invoke<Settings>('update_settings', { settings }),
  defaults: (action?: string) => invoke<Settings>('restore_shortcuts', { action }),
  recording: (active: boolean) => invoke<void>('record_shortcuts', { active }),
  onRecorded: (fn: (binding: string) => void) => listen<string>('shortcut-recorded', e => fn(e.payload)),
  overlay: () => invoke<OverlayData>('overlay_data'),
  select: (selection: { x?: number; y?: number; width?: number; height?: number; windowId?: string }) => invoke<void>('finish_selection', { selection }),
  cancel: () => invoke<void>('cancel_capture'),
  onState: (fn: (state: ShelfState) => void) => listen<ShelfState>('shelf-state', e => fn(e.payload)),
  onCollapsed: (fn: () => void) => listen('shelf-collapsed', fn),
  onCapture: (fn: (item: CaptureItem) => void) => listen<CaptureItem>('capture-created', e => fn(e.payload)),
  onLibraryChanged: (fn: () => void) => listen('library-changed', fn),
};
