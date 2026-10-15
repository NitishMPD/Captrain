const PATHS = {
  video: 'M3 6h12v12H3V6Zm12 4 6-3v10l-6-3',
  external: 'M14 4h6v6m0-6L10 14M10 4H4v16h16v-6',
  link: 'm10 14 4-4M8 16l-1 1a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m2 1 1-1a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0',
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14Zm9 16-4-4',
  settings: 'M4 7h9m4 0h3M4 17h3m4 0h9M15 4v6M9 14v6',
  back: 'm14 6-6 6 6 6',
  next: 'm10 6 6 6-6 6',
  camera: 'M4 8h3l1.5-2h7L17 8h3v11H4V8Zm8 8.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z',
  library: 'M5 5h6v6H5V5Zm8 0h6v6h-6V5ZM5 13h6v6H5v-6Zm8 0h6v6h-6v-6Z',
  list: 'M9 7h11M9 12h11M9 17h11M4.5 7h.01M4.5 12h.01M4.5 17h.01',
  home: 'M4 11 12 4l8 7v9h-5v-6H9v6H4v-9Z',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z',
  folderPlus: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Zm9 4v5m-2.5-2.5h5',
  folderPc: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Zm6.5 5.5 2.5 2.5 2.5-2.5M12 10v5',
  image: 'M4 5h16v14H4V5Zm0 11 4.5-4.5 3.5 3.5 3-3L20 16M9 9.5h.01',
  chevron: 'm7 14 5-5 5 5',
  down: 'm7 10 5 5 5-5',
  copy: 'M9 9h10v11H9V9Zm-4 7V4h10',
  save: 'M12 4v11m-4-4 4 4 4-4M5 20h14',
  trash: 'M5 7h14M10 7V4h4v3m-7 0 1 13h8l1-13',
  close: 'm6 6 12 12M18 6 6 18',
  plus: 'M12 5v14M5 12h14',
  minus: 'M5 12h14',
  fit: 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5',
  check: 'm5 12 4.5 4.5L19 7',
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d={PATHS[name]} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
