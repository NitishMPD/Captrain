import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export function ReleaseNotes({ notes }: { notes: string }) {
  return <div className="update-notes" role="region" aria-label="Release notes" tabIndex={0}>
    <Markdown remarkPlugins={[remarkGfm]} skipHtml disallowedElements={['img']}
      urlTransform={url => /^https?:\/\//i.test(url) ? url : ''}
      components={{ a: ({ children, href }) => href
        ? <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>
        : <span>{children}</span> }}>
      {notes}
    </Markdown>
  </div>;
}
