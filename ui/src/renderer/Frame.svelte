<script lang="ts">
  // A `frame` component without scripts: mail-like HTML in an opaque-origin sandboxed frame
  // with no scripts, forms or network beyond what its own policy allows. Links open outside.
  import { frameSpacing, textSpacing } from '../lib/text-spacing.svelte';
  let { title, doc, height = 'fill', designed = false, remote = false }: { title: string; doc: string; height?: unknown; designed?: boolean; remote?: boolean } = $props();
  const style = $derived(designed
    ? 'html{color-scheme:light}body{margin:24px;font:15px/1.6 system-ui,sans-serif;color:#1d1d1f;background:#fff;overflow-wrap:anywhere}'
    : 'html{color-scheme:dark}body{margin:0;font:15.5px/1.65 system-ui,sans-serif;color:#d5ddd7;background:transparent;overflow-wrap:anywhere}a{color:#a2d6b1}blockquote{margin:0 0 0 2px;padding-left:14px;border-left:1px solid #3a463f;color:#9faba3}');
  // Mail that paints its own colours keeps a light sheet; its dark-mode rules would then mix light
  // backgrounds with light text, so the media feature is renamed and never matches.
  const body = $derived(designed ? doc.replace(/prefers-color-scheme\s*:\s*dark/gi, 'prefers-color-scheme: colecto-light') : doc);
  const srcdoc = $derived(`<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:${remote ? ' https:' : ''}"><base target="_blank"><style>${style}${frameSpacing(textSpacing, designed)}img{max-width:100%;height:auto}</style>${body}`);
</script>

<iframe class="html-body" class:designed {title} sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" {srcdoc} style:height={typeof height === 'number' ? `${height}px` : undefined}></iframe>
