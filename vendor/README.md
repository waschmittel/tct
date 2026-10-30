# Vendored crates

## `tui-textarea-2` (fork)

Upstream `tui-textarea-2` has no hanging indent for soft-wrapped lines. Our fork adds
`TextArea::set_wrap_continuation_indent(Some(fn(&str) -> usize))`: continuation rows of a
logical line wrap to `width - indent` and render/navigate with that indent (used by the
description editor so list items wrap like the markdown preview).

The fork is **upstream + one patch**: `vendor/patches/tui-textarea-2.patch`
(touches `wrap.rs`, `screen_map.rs`, `widget.rs`, `textarea.rs`; all additions are marked
`TCT patch`). Never edit `vendor/tui-textarea-2` without regenerating the patch.

```sh
scripts/vendor-textarea.sh check            # vendor/ == upstream + patch? newer upstream?
scripts/vendor-textarea.sh refresh-patch    # after editing vendor/tui-textarea-2/src
scripts/vendor-textarea.sh update [VER]     # bump: re-vendor VER (default latest) + re-apply patch
```

Bumping upstream: `update` → resolve any `*.rej` → `refresh-patch` → `cargo test` → commit.
If upstream ever ships hanging indent, drop the fork: delete `vendor/`, restore the registry
dependency in `Cargo.toml`, and remove the `set_wrap_continuation_indent` call in
`src/insert/text_area_input.rs`.
