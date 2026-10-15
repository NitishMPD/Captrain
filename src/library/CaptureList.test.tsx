import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { CaptureList } from './CaptureList';
import type { CaptureItem } from '../native';

const item: CaptureItem = { id: 'link', kind: 'url', url: 'https://github.com/NitishMPD/Captrain', filename: 'saved.url', createdAt: '2026-10-15T10:00:00Z', width: 0, height: 0, folderId: null, exportedPath: null };
const render = (overrides: Partial<CaptureItem> = {}) => renderToStaticMarkup(
  <CaptureList items={[{ ...item, ...overrides }]} thumbs={{}} layout="list" onOpen={() => {}} menuFor={() => []} />,
);

describe('saved link rows', () => {
  it('shows the domain, full address and browser action without a duplicate badge', () => {
    const html = render();
    expect(html).toContain('>github.com</strong>');
    expect(html).toContain('class="link-address"');
    expect(html).toContain(item.url);
    expect(html).toContain('Open in browser');
    expect(html).toContain('class="link-open"');
    expect(html).not.toContain('class="badge"');
  });
  it('retains export badges and tolerates missing or malformed URLs', () => {
    expect(render({ exportedPath: 'C:/saved.url' })).toContain('class="badge"');
    expect(render({ url: null })).toContain('>saved.url</strong>');
    expect(render({ url: 'invalid' })).toContain('>invalid</strong>');
  });
  it('preserves screenshot filenames and badges', () => {
    const html = render({ kind: 'region', url: null, filename: 'capture.png' });
    expect(html).toContain('>capture.png</strong>');
    expect(html).toContain('class="badge"');
    expect(html).not.toContain('class="link-address"');
    expect(html).not.toContain('class="link-open"');
  });
});
