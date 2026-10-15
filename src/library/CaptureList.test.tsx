import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { CaptureList, linkAddress } from './CaptureList';
import type { CaptureItem } from '../native';

const item: CaptureItem = { id: 'link', kind: 'url', url: 'https://github.com/NitishMPD/Captrain', title: null, filename: 'saved.url', createdAt: '2026-10-15T10:00:00Z', width: 0, height: 0, folderId: null, exportedPath: null };
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


describe('Windows folder links and names', () => {
  it('shows a decoded local folder path with an Explorer action', () => {
    const html = render({ url: 'file:///D:/Nitish/GodotProjects/wise-warrior/' });
    expect(html).toContain('>wise-warrior</strong>');
    expect(html).toContain('D:\\Nitish\\GodotProjects\\wise-warrior\\');
    expect(html).toContain('Open in Explorer');
    expect(html).not.toContain('Open in browser');
  });
  it('decodes spaces, Unicode and network paths without changing web URLs', () => {
    expect(linkAddress('file:///C:/My%20Projects/%E6%B5%8B%E8%AF%95')).toBe('C:\\My Projects\\\u6d4b\u8bd5');
    expect(linkAddress('file://server/shared/My%20Projects')).toBe('\\\\server\\shared\\My Projects');
    expect(linkAddress('https://example.com/a%20b')).toBe('https://example.com/a%20b');
  });
  it('shows custom names while retaining the full target for both link types', () => {
    const web = render({ title: 'Build logs' });
    expect(web).toContain('>Build logs</strong>');
    expect(web).toContain(item.url);
    const folder = render({ title: 'Game source', url: 'file:///D:/Games/wise-warrior' });
    expect(folder).toContain('>Game source</strong>');
    expect(folder).toContain('D:\\Games\\wise-warrior');
  });
});
