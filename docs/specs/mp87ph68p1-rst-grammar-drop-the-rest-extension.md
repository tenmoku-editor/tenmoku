---
task_uid: "01m8d0xk2s7d6s03mp87ph68p1"
task: "T753"
rev: 1
type: "chore"
complexity: "xs"
criteria:
  - id: "C1"
    text: "extensions/grammars/rst/extension.toml lists extensions = [\"rst\"] and nothing else changes in that file"
    required: true
    manual: false
  - id: "C2"
    text: "cargo nextest run -p fe-syntax --test wasm_parity passes"
    required: true
    manual: false
---

# rst grammar: drop the `rest` extension, which the http grammar owns (Mac T2129 follow-up)

# Drop `rest` from the rst grammar's extensions: `*.rest` belongs to http

T749 (Mac T2129) gave `extensions/grammars/rst/extension.toml` the extensions
`["rst", "rest"]`. Mac main has since landed the HTTP request grammar
(`extensions/grammars/http`, Mac commit a22cc913f), which claims
`["http", "rest"]`: `.rest` is the REST Client / kulala.nvim request-file
extension. With both, `fe-syntax::wasm_parity every_extension_is_well_formed`
fails: "extensions/grammars: http and rst are each for *.rest". That blocks the
helium sync to mac/main 2524b311e (sync gates 01m8czck0a4c, 01m8d05zmqvj).

reStructuredText's own extension is `.rst`; nvim maps `.rest` to nothing. So
rst gives up `rest` and http keeps it.

## Change

- `extensions/grammars/rst/extension.toml`: `extensions = ["rst"]`.
- Nothing else. The sample, golden and wasm are unchanged, and THIRD_PARTY.md is
  not touched.

Helium main has no http grammar yet, so wasm_parity passes before and after
this change here. The conflict shows only once it is merged with mac/main.

## Implementation notes

C2, run on commit 6678b6742 (the manifest change):
`cargo nextest run -p fe-syntax --test wasm_parity` exited 0, with
"24 tests run: 24 passed, 1 skipped". The skipped test is
`every_extension_highlights_its_sample`, which is `#[ignore]`d because it needs
`scripts/build-grammar-wasm.sh`; the command in C2 does not select it.
