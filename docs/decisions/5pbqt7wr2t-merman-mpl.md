# T2181: merman needs three MPL-2.0 crates, which deny.toml rejects

T2181 (Mermaid fences drawn as diagrams through merman 0.11) assumes
`cargo deny check licenses bans sources` passes with deny.toml and
THIRD_PARTY.md unchanged (C5). It doesn't: `cargo deny --offline check
licenses` rejects three crates:

| Crate | Version | Licence | Why it's there |
|---|---|---|---|
| cssparser | 0.38.0 | MPL-2.0 | a required dependency of `merman-core` and `merman-render` |
| cssparser-macros | 0.7.1 | MPL-2.0 | cssparser's proc macros |
| dtoa-short | 0.3.5 | MPL-2.0 | cssparser's number formatting |

`cssparser` isn't optional in either merman crate (it's `[dependencies.cssparser]`
with no `optional = true`; merman parses Mermaid's `classDef`/`style` CSS with
it), so no merman feature set avoids it. With `default-features = false` and
only `svg`, `diagram-flowchart` and `diagram-sequence`, bans and sources pass,
and `cargo tree -e features` has no `layout-elk`.

These are Servo's crates, used unmodified from crates.io. deny.toml allows
MPL-2.0 only crate by crate, "after review", with shipped crates listed in
THIRD_PARTY.md's "Shipped MPL-2.0 dependencies (unmodified)" table, as
`dwrote` and `option-ext` are.

## Options

1. **Allow the three crates (recommended).** Add `{ allow = ["MPL-2.0"], crate
   = "cssparser" }`, the same for `cssparser-macros` and `dtoa-short`, to
   deny.toml's exceptions, and three rows to THIRD_PARTY.md's MPL table. Only
   the two compiled into the binary need rows: `cssparser-macros` is a proc
   macro, build-time only like `cbindgen`, so it gets a line like cbindgen's
   instead. This widens the card's scope to deny.toml and THIRD_PARTY.md and
   changes C5 to "passes with these three exceptions".
2. Drop merman and draw no diagrams. No other pure-Rust Mermaid renderer
   with flowchart and sequence parity is known; a JS engine is out by the
   spec.

## Ruling

The user chose option 1 on 2026-11-27 (relayed by the lead; T2181 rev 2):
the three crates are allowed as unmodified MPL-2.0 dependencies. deny.toml
has one exception per crate, commented like dwrote's, and THIRD_PARTY.md
lists cssparser and dtoa-short in its shipped MPL-2.0 table and
cssparser-macros as build-time only, like cbindgen. C5 now asks for exactly
those three exceptions and no other licence change.

`cargo tree -p fe-markdown-view | wc -l` is 1362 before and 1513 after.
