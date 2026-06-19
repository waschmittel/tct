# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`UBIQUITOUS_LANGUAGE.md`** at the repo root — this repo's domain glossary (the `CONTEXT.md` the skills refer to).
- **`docs/adr/`** — read ADRs that touch the area you're about to work in.

This is a single-context repo: one glossary at the root, one `docs/adr/`. If a file doesn't exist, **proceed silently** — don't flag its absence or suggest creating it. The producer skill (`/grill-with-docs`) updates the glossary and ADRs lazily when terms or decisions get resolved.

## File structure

```
/
├── UBIQUITOUS_LANGUAGE.md          ← domain glossary
├── docs/adr/
│   ├── 0001-board-editor-aggregate.md
│   ├── 0002-command-enum-scope.md
│   └── …
└── src/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `UBIQUITOUS_LANGUAGE.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/grill-with-docs`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
