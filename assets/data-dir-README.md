# tct data directory

This folder is a [tct](https://github.com/waschmittel/tct) data directory.
tct is a keyboard-driven terminal Kanban board that keeps all of its data as
plain, local JSON files — no server, no database, no account.

- `boards/<board>/board.json` — a board: its lists and labels
- `boards/<board>/card-<id>.json` — one file per card
- `.version` — the storage format version

The files are meant to be diffed, committed and synced like any other text.
Edit boards and cards with `tct` rather than by hand.

Project page and documentation: https://github.com/waschmittel/tct
