/**
 * A small, safe Markdown parser for chat replies. It produces a tree that React
 * renders as elements, so model output is never injected as HTML.
 * Supported: paragraphs, #-headings, fenced code, lists, quotes, rules, and
 * inline **bold**, *italic*, `code` and [links](https://…). Anything unfinished
 * (common while a reply is still streaming) degrades to plain text.
 */

export type Inline =
  | { t: 'text'; s: string }
  | { t: 'b'; c: Inline[] }
  | { t: 'i'; c: Inline[] }
  | { t: 'code'; s: string }
  | { t: 'a'; href: string; c: Inline[] };

export type Block =
  | { t: 'p'; c: Inline[] }
  | { t: 'h'; level: 1 | 2 | 3; c: Inline[] }
  | { t: 'code'; lang: string; s: string }
  | { t: 'ul' | 'ol'; items: Inline[][] }
  | { t: 'quote'; c: Inline[] }
  | { t: 'hr' };

const SAFE_URL = /^(https?:\/\/|mailto:)[^\s<>"]+$/i;

export function isSafeUrl(u: string): boolean {
  return SAFE_URL.test(u);
}

/** Parse inline spans. Unmatched markers stay as literal text. */
export function parseInline(src: string): Inline[] {
  const out: Inline[] = [];
  let buf = '';
  const flush = () => {
    if (buf) out.push({ t: 'text', s: buf });
    buf = '';
  };
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (ch === '`') {
      const end = src.indexOf('`', i + 1);
      if (end > i + 1) {
        flush();
        out.push({ t: 'code', s: src.slice(i + 1, end) });
        i = end + 1;
        continue;
      }
    } else if (ch === '*' && src[i + 1] === '*') {
      let end = src.indexOf('**', i + 2);
      while (end > 0 && src[end + 2] === '*') end++; // "***": the closing pair is the last two stars
      if (end > i + 2) {
        flush();
        out.push({ t: 'b', c: parseInline(src.slice(i + 2, end)) });
        i = end + 2;
        continue;
      }
    } else if ((ch === '*' || ch === '_') && src[i + 1] !== ch && src[i + 1] !== ' ' && src[i + 1] !== undefined) {
      // _ only counts at a word start, so snake_case stays intact
      const startOk = ch === '*' || i === 0 || /[\s(]/.test(src[i - 1]);
      const end = startOk ? src.indexOf(ch, i + 1) : -1;
      if (end > i + 1 && src[end - 1] !== ' ' && (ch === '*' || end === src.length - 1 || /[\s.,;:!?)]/.test(src[end + 1]))) {
        flush();
        out.push({ t: 'i', c: parseInline(src.slice(i + 1, end)) });
        i = end + 1;
        continue;
      }
    } else if (ch === '[') {
      const close = src.indexOf('](', i + 1);
      const paren = close > 0 ? src.indexOf(')', close + 2) : -1;
      if (close > i && paren > close) {
        const href = src.slice(close + 2, paren).trim();
        if (isSafeUrl(href)) {
          flush();
          out.push({ t: 'a', href, c: parseInline(src.slice(i + 1, close)) });
          i = paren + 1;
          continue;
        }
      }
    }
    buf += ch;
    i++;
  }
  flush();
  return out;
}

const FENCE = /^\s*```\s*([\w+#.-]*)\s*$/;
const HEADING = /^(#{1,3})\s+(.*)$/;
const UL = /^\s*[-*+]\s+(.*)$/;
const OL = /^\s*\d+[.)]\s+(.*)$/;
const QUOTE = /^\s*>\s?(.*)$/;
const HR = /^\s*([-*_])(\s*\1){2,}\s*$/;

export function parseMarkdown(src: string): Block[] {
  const lines = src.replace(/\r\n?/g, '\n').split('\n');
  const blocks: Block[] = [];
  let i = 0;
  let para: string[] = [];
  const flushPara = () => {
    if (para.length) blocks.push({ t: 'p', c: parseInline(para.join('\n')) });
    para = [];
  };

  while (i < lines.length) {
    const line = lines[i];
    const fence = FENCE.exec(line);
    if (fence) {
      flushPara();
      const body: string[] = [];
      i++;
      while (i < lines.length && !FENCE.test(lines[i])) body.push(lines[i++]);
      i++; // closing fence (or end of a still-streaming block)
      blocks.push({ t: 'code', lang: fence[1], s: body.join('\n') });
      continue;
    }
    if (!line.trim()) {
      flushPara();
      i++;
      continue;
    }
    const h = HEADING.exec(line);
    if (h) {
      flushPara();
      blocks.push({ t: 'h', level: h[1].length as 1 | 2 | 3, c: parseInline(h[2]) });
      i++;
      continue;
    }
    if (HR.test(line)) {
      flushPara();
      blocks.push({ t: 'hr' });
      i++;
      continue;
    }
    const list = UL.test(line) ? UL : OL.test(line) ? OL : null;
    if (list) {
      flushPara();
      const items: Inline[][] = [];
      while (i < lines.length && list.test(lines[i])) items.push(parseInline(list.exec(lines[i++])![1]));
      blocks.push({ t: list === UL ? 'ul' : 'ol', items });
      continue;
    }
    if (QUOTE.test(line)) {
      flushPara();
      const q: string[] = [];
      while (i < lines.length && QUOTE.test(lines[i])) q.push(QUOTE.exec(lines[i++])![1]);
      blocks.push({ t: 'quote', c: parseInline(q.join('\n')) });
      continue;
    }
    para.push(line);
    i++;
  }
  flushPara();
  return blocks;
}
