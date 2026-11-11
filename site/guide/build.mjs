// Renders docs/user-guide.md to <outdir>/guide/index.html (+ mermaid bundle).
// Usage: node build.mjs <outdir>
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import { Marked } from "marked";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(process.argv[2] ?? "public", "guide");
const REPO = "https://github.com/waschmittel/tct";
const md = readFileSync(join(here, "../../docs/user-guide.md"), "utf8");

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("json", json);
hljs.registerLanguage("md", markdown);

const esc = s => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const plain = s => s.replace(/<[^>]+>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#39;/g, "'");
const slug = s => plain(s).toLowerCase().replace(/[^\p{L}\p{N}\s-]/gu, "").trim().replace(/\s/g, "-");

// Mermaid diagrams become inline SVG here, once per colour scheme, with
// mermaid-rs-renderer (`mmdr`, pure Rust, no browser). Set MMDR to its path.
const mmdr = process.env.MMDR ?? "mmdr";
let diagrams = 0;
function diagram(source) {
  const n = diagrams++;
  const text = source.replace(/<\/?i>/g, ""); // mmdr prints inline <i> literally
  const [light, dark] = ["neutral", "dark"].map(theme => {
    const svg = execFileSync(mmdr, ["-i", "-", "-t", theme, "-e", "svg"], { input: text, encoding: "utf8", maxBuffer: 1 << 26 });
    // Unique ids per diagram and scheme: inline SVGs share one document.
    return svg.replace(/<\?xml[^>]*>\s*/, "")
      // drop the opaque background rect: the card behind the figure shows through
      .replace(/(<svg[^>]*>)<rect x="[^"]*" y="[^"]*" width="[^"]*" height="[^"]*" fill="#[0-9a-f]+"\/>/i, "$1").replace(/\bid="([^"]+)"/g, `id="d${n}-${theme}-$1"`).replace(/url\(#([^)]+)\)/g, `url(#d${n}-${theme}-$1)`).replace(/href="#([^"]+)"/g, `href="#d${n}-${theme}-$1"`);
  });
  return `<figure class="diagram"><div class="light">${light}</div><div class="dark">${dark}</div></figure>\n`;
}

const toc = [];
const seen = new Map();
let title = "tct User Guide";

const marked = new Marked({
  gfm: true,
  renderer: {
    heading({ tokens, depth }) {
      const html = this.parser.parseInline(tokens);
      if (depth === 1) { title = plain(html); return `<h1>${html}</h1>\n`; }
      let id = slug(html);
      const n = seen.get(id) ?? 0;
      seen.set(id, n + 1);
      if (n) id += `-${n}`;
      if (depth <= 3) toc.push({ depth, id, text: plain(html) });
      return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-label="Link to this section">#</a>${html}</h${depth}>\n`;
    },
    code({ text, lang }) {
      if (lang === "mermaid") return diagram(text);
      const known = lang && hljs.getLanguage(lang);
      const body = known ? hljs.highlight(text, { language: lang }).value : esc(text);
      return `<div class="code"><pre><code class="hljs${lang ? ` language-${esc(lang)}` : ""}">${body}</code></pre><button type="button" class="copy" aria-label="Copy code">copy</button></div>\n`;
    },
    table(token) {
      const head = token.header.map((c, i) => `<th${token.align[i] ? ` align="${token.align[i]}"` : ""}>${this.parser.parseInline(c.tokens)}</th>`).join("");
      const rows = token.rows.map(r => `<tr>${r.map((c, i) => `<td${token.align[i] ? ` align="${token.align[i]}"` : ""}>${this.parser.parseInline(c.tokens)}</td>`).join("")}</tr>`).join("\n");
      return `<div class="tablewrap"><table><thead><tr>${head}</tr></thead><tbody>\n${rows}\n</tbody></table></div>\n`;
    },
    link({ href, title: t, tokens }) {
      let url = href;
      // Links to other repo files (docs/…) point at GitHub; #anchors stay local.
      if (!/^(https?:|mailto:|#)/.test(url)) {
        const path = join("docs", url).replace(/\\/g, "/");
        url = `${REPO}/${url.endsWith("/") ? "tree" : "blob"}/main/${path}`;
      }
      return `<a href="${esc(url)}"${t ? ` title="${esc(t)}"` : ""}>${this.parser.parseInline(tokens)}</a>`;
    },
  },
});

const body = marked.parse(md);

const nav = [];
let open = false;
for (const h of toc) {
  if (h.depth === 2) {
    if (open) nav.push("</ul></li>");
    nav.push(`<li class="grp"><a href="#${h.id}">${esc(h.text)}</a><ul>`);
    open = true;
  } else nav.push(`<li><a href="#${h.id}">${esc(h.text)}</a></li>`);
}
if (open) nav.push("</ul></li>");

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="The complete tct user guide: the TUI, the editor, the CLI, working with AI agents, sync and data formats.">
<meta name="theme-color" content="#14141e">
<link rel="icon" href="../icon.svg" type="image/svg+xml">
<link rel="stylesheet" href="../style.css">
<link rel="stylesheet" href="guide.css">
</head>
<body class="guide">
<nav><div class="wrap">
  <a class="brand" href="../"><img src="../icon.svg" alt="" width="26" height="26"><span>tct<span>_</span></span></a>
  <a class="l" href="../">Home</a>
  <a class="l" href="./" aria-current="page">User guide</a>
  <a class="l" href="${REPO}">GitHub</a>
  <button type="button" id="menu" aria-label="Contents" aria-expanded="false">☰</button>
</div></nav>
<div class="layout">
  <aside id="side" aria-label="Contents">
    <input id="filter" type="search" placeholder="Filter sections…" aria-label="Filter sections" autocomplete="off">
    <ul id="toc">
${nav.join("\n")}
    </ul>
  </aside>
  <main id="doc">
${body}
    <p class="edit"><a href="${REPO}/edit/main/docs/user-guide.md">Edit this page on GitHub</a></p>
  </main>
</div>
<script src="guide.js"></script>
</body>
</html>
`;

mkdirSync(out, { recursive: true });
writeFileSync(join(out, "index.html"), page);
for (const f of ["guide.css", "guide.js"]) copyFileSync(join(here, f), join(out, f));
console.log(`guide: ${toc.length} headings, ${diagrams} diagrams -> ${out}`);
