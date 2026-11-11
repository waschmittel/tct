Ship **Release 2.0** by the end of Q2. This card is the single source of truth: scope, owners and schedule live here, and the checklist below tracks what is left.

> [!warning]
> The feature freeze on **May 15** is firm. Anything not merged by then moves to 2.1.

# Goals

- Ship the new **export** pipeline[^pdf]
- Cut start-up time in half
  - measured on the reference laptop
  - reported in the release notes
- Zero data migrations[^fmt]

# Schedule

| Phase  |  Owner  | Weeks |
|:-------|:-------:|------:|
| Design | Ada     |     2 |
| Build  | Grace   |     6 |
| Verify | Linus   |     3 |

# Responsibilities

|       | Design | Build   | Verify  |
|-------|:------:|:-------:|:-------:|
| Ada   | owner  | review  | -       |
| Grace | review | owner   | support |
| Linus | -      | support | owner   |

# Packaging

```
cargo build --release
./scripts/package.sh
```

>>> [!tip]
Dry-run the packaging script on a clean checkout first.

It catches missing files before the tag does.
>>>

[^pdf]: PDF/A-2b, fonts embedded, so it prints the same everywhere.
[^fmt]: The on-disk format stays at version 1; old data dirs open unchanged.
