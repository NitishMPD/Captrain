const PATHS = {
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14Zm9 16-4-4',
  settings: 'M4 7h9m4 0h3M4 17h3m4 0h9M15 4v6M9 14v6',
  back: 'm14 6-6 6 6 6',
  camera: 'M4 8h3l1.5-2h7L17 8h3v11H4V8Zm8 8.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z',
  library: 'M5 5h6v6H5V5Zm8 0h6v6h-6V5ZM5 13h6v6H5v-6Zm8 0h6v6h-6v-6Z',
  chevron: 'm7 14 5-5 5 5',
  copy: 'M9 9h10v11H9V9Zm-4 7V4h10',
  save: 'M12 4v11m-4-4 4 4 4-4M5 20h14',
  trash: 'M5 7h14M10 7V4h4v3m-7 0 1 13h8l1-13',
  close: 'm6 6 12 12M18 6 6 18',
} as const;

export function Icon({ name }: { name: keyof typeof PATHS }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d={PATHS[name]} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
