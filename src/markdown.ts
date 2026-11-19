import MarkdownIt from 'markdown-it';
import hljs from 'highlight.js/lib/common';
import 'highlight.js/styles/github.css';

const md: MarkdownIt = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  highlight(str: string, lang: string): string {
    let code: string;
    if (lang && hljs.getLanguage(lang)) {
      try {
        code = hljs.highlight(str, { language: lang, ignoreIllegals: true }).value;
      } catch {
        code = md.utils.escapeHtml(str);
      }
    } else {
      code = md.utils.escapeHtml(str);
    }
    const label = md.utils.escapeHtml(lang || 'text');
    return `<div class="code-wrap"><button class="copy-code">复制</button><pre><code class="hljs" data-lang="${label}">${code}</code></pre></div>`;
  },
});

export function renderMarkdown(content: string): string {
  return md.render(content);
}

export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
  }
}
