import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ReleaseNotes } from './ReleaseNotes';

describe('release notes', () => {
  it('renders GitHub headings, lists, emphasis and bare changelog URLs', () => {
    const url = 'https://github.com/NitishMPD/Captrain/compare/v0.1.11...v0.1.12';
    const html = renderToStaticMarkup(<ReleaseNotes notes={`## Changes\n\n- Fixed *formatting*\n\n**Full Changelog**: ${url}`} />);
    expect(html).toContain('<h2>Changes</h2>');
    expect(html).toContain('<li>Fixed <em>formatting</em></li>');
    expect(html).toContain('<strong>Full Changelog</strong>');
    expect(html).toContain(`href="${url}" target="_blank" rel="noopener noreferrer"`);
    expect(html).not.toContain('**');
    expect(html).toContain('tabindex="0"');
  });

  it('blocks raw HTML, remote images and unsafe link protocols', () => {
    const html = renderToStaticMarkup(<ReleaseNotes notes={'<script>alert(1)</script>\n\n<img src="https://example.com/tracker">\n\n![tracker](https://example.com/image)\n\n[unsafe](javascript:alert%281%29)\n\n[local](file:///C:/secret)'} />);
    expect(html).not.toMatch(/<script|<img|href=|javascript:|file:/);
    expect(html).toContain('<span>unsafe</span>');
    expect(html).toContain('<span>local</span>');
  });
});
