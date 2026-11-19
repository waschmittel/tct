# Website: no horizontal page scroll at any viewport width

Neither the landing page (`/`) nor the rendered user guide (`/guide/`) may ever allow horizontal page scrolling (`document.documentElement.scrollWidth <= window.innerWidth`) at any viewport width down to 320px.

## Why

Full-bleed section backgrounds on the landing page previously used `section:nth-of-type(even)::before` and `section[style]::before` with `position: absolute; inset: 0 -100vmax;` inside `<main class="wrap">`, paired only with `body { overflow-x: hidden; }`. On mobile browsers (iOS Safari, Android Chrome), `overflow-x: hidden` on `body` alone does not prevent touch-panning into `100vmax` of overflow outside the viewport, letting users swipe completely sideways off the page content. It also disabled `position: sticky` on `nav` and `#side`.

Secondary causes compounded narrow-screen overflow:
- A `@media (max-width: 760px)` block for mobile `nav` appeared before later `nav a.l` and `nav a.l[style] svg` rules in `site/style.css`, which overrode mobile padding and icon sizes, causing the nav bar to exceed 320–360px.
- Bare `1fr` CSS Grid tracks (`minmax(auto, 1fr)` by spec) allowed unbreakable children to widen tracks beyond the viewport.
- Inline `<code>` lacked `overflow-wrap: anywhere`, and `.window pre` overrode `.copy pre`'s right padding and `#install pre`'s small-screen wrapping rule.

## Decision

1. **Full-bleed sections via layout, never negative breakouts:** `<main>` has no `.wrap`; each `<section>` spans the full viewport width and wraps its content in `<div class="wrap">`. Section backgrounds sit directly on `<section>`. Never use `-100vmax`, `100vw`, or negative horizontal `inset` tricks to break out of a container.
2. **Root clipping without breaking sticky:** Set `overflow-x: clip` (never `overflow-x: hidden`) on both `html` and `body`, alongside `body { overflow-wrap: break-word; }` and `:not(pre) > code { overflow-wrap: anywhere; }`.
3. **Contain wide elements locally:** Wide blocks (carousel `.track` with `overscroll-behavior-x: contain`, `.window pre`, `.code pre`, `.tablewrap`, `.diagram`) scroll horizontally inside their own bounded box (`overflow-x: auto`) rather than expanding the page.
4. **Defensive CSS Grid tracks:** Use `minmax(0, ...)` (or `minmax(min(14rem, 100%), 1fr)`) on all `grid-template-columns` tracks so grid children can shrink below `min-content`.
5. **Cascade-safe mobile nav down to 320px:** Keep mobile `@media (max-width: ...)` rules for `nav` after all base `nav` rules in `site/style.css` and size them to fit comfortably at 320px on both `/` and `/guide/`.

## Considered and rejected

- **`overflow-x: hidden` on `html, body`:** Clips overflow in some browsers, but disables `position: sticky` (`nav`, `#side`) by creating a scroll container on `html`/`body` and masks underlying layout overflow rather than fixing it.
- **`width: 100vw` full-bleed bands:** Includes vertical scrollbar width on Windows/Linux desktops, recreating horizontal overflow.
- **Disabling overflow silently without changing `<main>`:** Leaves the `-100vmax` root cause intact and keeps mobile `nav` / grid overflow hidden behind clipping.

## Consequences

- `site/layout.test.mjs` (`node --test site/layout.test.mjs`) enforces these static invariants (no `100vmax`/`100vw`, no `overflow-x: hidden`, `overflow-x: clip` on `html` and `body`, no bare `fr` tracks in `grid-template-columns`, full-width `<section>` + inner `.wrap`, viewport meta, mobile `nav` cascade order) and runs in `.github/workflows/pages.yml` on every Pages build.
- Any new page section or full-bleed band on the website must wrap its inner content in `.wrap` rather than breaking out of a narrow parent.
