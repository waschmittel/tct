// Run with: node --test site/layout.test.mjs
// Guards against horizontal page overflow regressions on the project website (ADR-0024).
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = rel => readFileSync(join(root, rel), "utf8");
const stripCssComments = css => css.replace(/\/\*[\s\S]*?\*\//g, "");

function stripMinmax(value) {
  let out = "";
  let i = 0;
  const lower = value.toLowerCase();
  while (i < value.length) {
    if (lower.startsWith("minmax(", i)) {
      let depth = 1;
      i += 7;
      while (i < value.length && depth > 0) {
        if (value[i] === "(") depth++;
        else if (value[i] === ")") depth--;
        i++;
      }
      out += " ";
    } else {
      out += value[i++];
    }
  }
  return out;
}

const cssFiles = ["site/style.css", "site/guide/guide.css"];

test("no 100vmax or 100vw breakout sizing in website CSS", () => {
  for (const file of cssFiles) {
    const css = stripCssComments(read(file));
    assert.doesNotMatch(
      css,
      /\b100v(max|w)\b/i,
      `${file} must not use 100vmax/100vw (causes horizontal scroll on mobile)`
    );
  }
});

test("website CSS never uses overflow-x: hidden (breaks sticky and fails on mobile body)", () => {
  for (const file of cssFiles) {
    const css = stripCssComments(read(file));
    assert.doesNotMatch(
      css,
      /overflow-x\s*:\s*hidden\b/i,
      `${file} must use overflow-x: clip on root or overflow-x: auto on internal scrollers`
    );
  }
});

test("site/style.css sets overflow-x: clip on both html and body", () => {
  const css = stripCssComments(read("site/style.css"));
  assert.match(css, /\bhtml\s*\{[^}]*overflow-x\s*:\s*clip\b/i);
  assert.match(css, /\bbody\s*\{[^}]*overflow-x\s*:\s*clip\b/i);
});

test("all CSS grid-template-columns avoid bare fr tracks outside minmax()", () => {
  for (const file of cssFiles) {
    const css = stripCssComments(read(file));
    for (const match of css.matchAll(/grid-template-columns\s*:\s*([^;}]+)/gi)) {
      const stripped = stripMinmax(match[1]);
      assert.doesNotMatch(
        stripped,
        /(?<![\w-])(?:\d+(?:\.\d+)?|\.\d+)fr\b/i,
        `${file} has bare fr track in: ${match[0]}`
      );
    }
  }
});

test("landing page uses full-width <section> with inner .wrap instead of <main class=\"wrap\">", () => {
  const html = read("site/index.html");
  assert.match(html, /<main>/);
  assert.doesNotMatch(html, /<main\b[^>]*class="[^"]*\bwrap\b/);

  const sections = [...html.matchAll(/<section\b[^>]*>(\s*<div class="wrap">)?/g)];
  assert.ok(sections.length >= 5, "expected landing page sections");
  for (const m of sections) {
    assert.ok(m[1], `section must immediately open <div class="wrap">: ${m[0]}`);
  }

  const closes = [...html.matchAll(/<\/div><\/section>/g)];
  assert.equal(closes.length, sections.length, "each section must close its inner .wrap div");
});

test("both landing page and user guide declare responsive viewport meta", () => {
  const vp = /<meta\s+name="viewport"\s+content="width=device-width,\s*initial-scale=1"\s*>/;
  assert.match(read("site/index.html"), vp);
  assert.match(read("site/guide/build.mjs"), vp);
});

test("mobile nav media queries are not overridden by later non-media nav rules", () => {
  const css = stripCssComments(read("site/style.css"));
  const mediaBlocks = [];
  let withoutMedia = "";
  let i = 0;
  while (i < css.length) {
    if (css.startsWith("@media", i)) {
      const open = css.indexOf("{", i);
      if (open === -1) break;
      let depth = 1;
      let j = open + 1;
      while (j < css.length && depth > 0) {
        if (css[j] === "{") depth++;
        else if (css[j] === "}") depth--;
        j++;
      }
      mediaBlocks.push({
        index: i,
        header: css.slice(i, open),
        body: css.slice(open + 1, j - 1),
      });
      i = j;
    } else {
      withoutMedia += css[i];
      i++;
    }
  }

  const baseMatches = [...withoutMedia.matchAll(/\bnav(?:\s+|\.)[^\n{]*\{/g)];
  assert.ok(baseMatches.length > 0, "expected base nav rules");
  const lastBaseSnippet = baseMatches[baseMatches.length - 1][0];
  const lastBaseNavPos = css.lastIndexOf(lastBaseSnippet);

  const mobileNavMedia = mediaBlocks.filter(
    b => /\(\s*max-width\s*:/i.test(b.header) && /\bnav\b/.test(b.body)
  );
  assert.ok(mobileNavMedia.length > 0, "expected mobile nav @media rule");
  for (const b of mobileNavMedia) {
    assert.ok(
      b.index > lastBaseNavPos,
      `mobile nav @media rule (${b.header.trim()}) at offset ${b.index} appears before base nav rule '${lastBaseSnippet}' at ${lastBaseNavPos}`
    );
  }
});
