import { useMemo, useState, type ReactNode } from 'react';
import { parseMarkdown, type Block, type Inline } from './markdown';

function host(u: string): string {
  try {
    return new URL(u).host;
  } catch {
    return u;
  }
}

function inline(nodes: Inline[]): ReactNode[] {
  return nodes.map((n, i) => {
    switch (n.t) {
      case 'text':
        return n.s;
      case 'b':
        return <strong key={i}>{inline(n.c)}</strong>;
      case 'i':
        return <em key={i}>{inline(n.c)}</em>;
      case 'code':
        return <code key={i} className="md-code">{n.s}</code>;
      case 'a':
        // Links are shown, not followed: VEX has no way to open a browser yet, and a chat reply
        // should never navigate the overlay. The address is visible so it can be copied.
        return (
          <span key={i} className="md-link" title={n.href}>
            {inline(n.c)}
            <small> ({host(n.href)})</small>
          </span>
        );
    }
  });
}

function CodeBlock({ lang, s }: { lang: string; s: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(s);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch {
      /* clipboard may be unavailable; selecting the text still works */
    }
  };
  return (
    <div className="md-pre">
      <div className="md-pre-bar">
        <span>{lang || 'code'}</span>
        <button type="button" className="link-btn" onClick={copy} data-testid="copy-code">{copied ? 'Copied' : 'Copy'}</button>
      </div>
      <pre><code>{s}</code></pre>
    </div>
  );
}

function block(b: Block, i: number): ReactNode {
  switch (b.t) {
    case 'p':
      return <p key={i}>{inline(b.c)}</p>;
    case 'h':
      return <p key={i} className={`md-h md-h${b.level}`}>{inline(b.c)}</p>;
    case 'code':
      return <CodeBlock key={i} lang={b.lang} s={b.s} />;
    case 'ul':
      return <ul key={i}>{b.items.map((it, j) => <li key={j}>{inline(it)}</li>)}</ul>;
    case 'ol':
      return <ol key={i}>{b.items.map((it, j) => <li key={j}>{inline(it)}</li>)}</ol>;
    case 'quote':
      return <blockquote key={i}>{inline(b.c)}</blockquote>;
    case 'hr':
      return <hr key={i} />;
  }
}

export function Markdown({ text }: { text: string }) {
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  return <div className="md">{blocks.map(block)}</div>;
}
