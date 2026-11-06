# Italic is `Ctrl+E`, not `Ctrl+I`

The description editor toggles bold on `Ctrl+B` and italic on `Ctrl+E`. `Cmd+B`/`Cmd+E` (and `Cmd+I`) work on terminals that report the Super modifier.

## Why

A terminal transmits `Ctrl+I` as the byte 0x09 — the same byte as `Tab`. Without the kitty keyboard protocol the application cannot tell them apart, so a `Ctrl+I` binding never fires and `Tab` (cell/list navigation) must keep working. `Ctrl+E` is free in the editor, and "emphasis" (`<em>` next to `<strong>` for bold) makes it memorable.

## Considered and rejected

- **`Ctrl+I`.** Indistinguishable from Tab on most terminals.
- **`Ctrl+U`, `Ctrl+O`, `Ctrl+J`.** `Ctrl+J` is a newline byte; `Ctrl+U`/`Ctrl+O` carry no mnemonic for italic.
- **Requiring the kitty keyboard protocol.** The Linux console and many multiplexers lack it (see the `TERM=linux` tier).

## Consequences

- `Ctrl+E` no longer reaches the textarea's "end of line"; `End` still does.
- The handler also accepts `i` with Cmd, so `Cmd+I` works where Super is reported; plain `Ctrl+I` stays Tab.
