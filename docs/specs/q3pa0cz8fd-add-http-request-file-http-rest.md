---
task_uid: "01m7kpwz6g80s4jvq3pa0cz8fd"
task: "T375"
rev: 1
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "extensions/grammars/http/ holds the file set every_extension_is_well_formed requires, grammar-source.toml pinned in the git form to https://github.com/dont-be-evil-company/tree-sitter-kulala-http at a full 40-hex commit (1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723 or newer) with grammar kulala_http and parity_needs [\"graphql\"], the MIT LICENSE verbatim, and extension.toml with packs [\"web\"]; nothing else changes except one appended THIRD_PARTY.md WebAssembly row and an http NOT_RELEASED row in grammar_pack_recorded.rs"
    required: true
    manual: false
  - id: "C2"
    text: "scripts/build-grammar-wasm.sh http passes on the lane's host: the wasm builds and loads within MAX_GRAMMAR_BYTES, and wasm_parity's ignored every_extension_highlights_its_sample passes for http, with test/sample.http (### separators, # @name metadata, @var = value, {{var}} and {{$uuid}}, query continuation lines, headers, JSON, XML, GraphQL, form and multipart bodies, \u003c file bodies, \u003c {% %} and \u003e {% %} scripts, \u003e\u003e redirects, run and import, a pasted response) parsing with no ERROR or MISSING node"
    required: true
    manual: false
  - id: "C3"
    text: "wasm_parity's every_extension_is_well_formed and fastedit's grammar_install_tests::every_first_party_manifest_parses pass with http present, its extension.toml giving extensions http and rest, line_comment #, no shebangs and no clash with a built-in"
    required: true
    manual: false
  - id: "C4"
    text: "test/sample.http.golden colours methods, URLs, HTTP versions, header names and values, variables and their declarations, metadata, comments, separators and status lines, and colours the JSON, XML, GraphQL and JavaScript parts by their injected languages; injections.scm injects only json, xml, graphql and javascript"
    required: true
    manual: false
  - id: "C5"
    text: "cargo nextest run -p fe-syntax -p fastedit passes, including grammar_pack_recorded"
    required: true
    manual: false
---

# Add HTTP request file (.http, .rest; JetBrains HTTP Client and REST Client) syntax highlighting as a first-party WebAssembly grammar (extensions/grammars/http) (Mac T1901)

# HTTP request files (.http, .rest) as a first-party WebAssembly grammar

The user asked (2026-11-16): "Procfiles don't seem to be syntax highlighted.
Are there any other file formats like this that we're missing that are common
within certain types of projects?" The lead proposed HTTP client request
files (JetBrains HTTP Client / VS Code REST Client `.http` and `.rest` files,
common in API projects) among others and the user said "Go for it".

One new directory, `extensions/grammars/http/`, following
`extensions/grammars/README.md` and the Nim card T1629 (landed 96836cb2b:
`git show 96836cb2b` is the model, file for file).

## This language

- Grammar: https://github.com/dont-be-evil-company/tree-sitter-kulala-http
  (MIT; the grammar of kulala.nvim, whose org was renamed from mistweaverco),
  directory `src`. Pin: the head of `main`,
  1e238f6d34cf1b8c9cecedf78c073d8c6a3c8723 (2026-10-21; last grammar change
  bb70a577, 2026-07-24), or a newer commit that builds and passes. Generated
  `parser.c` (756 KB, LANGUAGE_VERSION 15) is committed; no external scanner.
  Export `tree_sitter_kulala_http`, so `grammar = "kulala_http"` and
  `wasm = "tree-sitter-kulala-http.wasm"`, while the directory and
  `language` are `http` (a language may differ from its export, as `erb`
  and `ejs` share `embedded_template`).
- Why not rest-nvim/tree-sitter-http (MIT, nvim-treesitter's `http`
  parser): it's the base kulala's grammar grew from, but its last grammar
  change was 2025-03 and it is LANGUAGE_VERSION 14, while kulala's covers more
  of the JetBrains format. Use rest-nvim only if kulala's fails to build or
  load; say so in the decision file and the THIRD_PARTY row.
- JetBrains/REST Client features the kulala grammar parses, all to be in the
  sample: `###` request separators with an optional name; `# @name x` and
  other `# @` metadata; `//` and `#` comments; `@host = example.com` file
  variables; `{{host}}` and `{{$uuid}}` variables in URLs, headers and
  bodies; request lines (`GET`, `POST`, ..., an optional `HTTP/1.1`) with
  query strings and indented `?`/`&` continuation lines; headers; JSON, XML,
  GraphQL (`X-REQUEST-TYPE: GraphQL`, with a JSON variables part),
  `application/x-www-form-urlencoded` and multipart bodies; `< ./body.json`
  external bodies; `< {% ... %}` pre-request and `> {% ... %}` response handler
  scripts and `> ./handler.js` paths; `>> ./out.json` / `>>! ./out.json`
  response redirects; `run #name` and `import ./other.http` commands; and a
  pasted response (`HTTP/1.1 200 OK`). No fork expected: if ordinary
  JetBrains or REST Client files give an ERROR node, block the card with the
  snippet and its parse tree (a patched fork is a user decision, as Nim's was).
- Licence: MIT (`LICENSE`, "Copyright (c) 2026+ The Don't Be Evil Company").
- File extensions: `http`, `rest`. File names: none. Shebangs: none.
- Comments: line `#` (the manifest takes one; `//` also comments).
- Queries: upstream `queries/kulala_http/highlights.scm` and
  `injections.scm` (MIT, same repository, nvim style, so
  `earlier_patterns_win = false`): adapt both. Its captures include
  non-standard ones (`@query_param.name`, `@form_param_name`,
  `@redirect_path`, `@external_body_path`, `@spell`, ...): map each to
  fe-syntax's vocabulary (property, string, markup.link.url, variable,
  function.method, keyword, ...) or drop it.
- Injections: keep `json_body` -> `json`, `xml_body` -> `xml`,
  `graphql_data` -> `graphql` and the script body -> `javascript` (its
  `#set! injection.include-children` is supported). Drop the `comment` and
  `jq` (`kulala-jq` metadata) injections: no such language here. GraphQL is
  a first-party extension, so `grammar-source.toml` gets
  `parity_needs = ["graphql"]`; json, xml and javascript are built-ins.
- Version: `0.2.4` (upstream `tree-sitter.json`).
- Packs: `packs = ["web"]`.

## Requirements

- `grammar-source.toml` in the git form (`git`, `rev` = full 40-hex commit,
  `src`, `grammar`, `parity_needs`), with Nim's comment lines.
- `LICENSE` copied verbatim from the grammar at that commit.
- `extension.toml`: `id = "tenmoku.grammar_http"`,
  `name = "HTTP request syntax"`, `version`, `license = "MIT"`,
  `[engine] tenmoku = "1.0"`, one `[[contributes.grammars]]` with
  `language = "http"`, the wasm, `extensions = ["http", "rest"]`,
  `highlights`, `injections`, `packs`, `earlier_patterns_win`,
  `license = "MIT"`, `line_comment = "#"`. No other keys.
- `highlights.scm` and `injections.scm`: self-contained; a header comment
  says where each came from, at which commit, under what licence, and what
  was changed. Drop patterns using predicates fe-syntax doesn't support.
- `test/sample.http`: 60-140 lines touching every feature listed above,
  parsing with no ERROR or MISSING node. Generate `test/sample.http.golden`
  with `UPDATE_GOLDEN=1 scripts/build-grammar-wasm.sh http`, then read it:
  methods, URLs, versions, header names and values, variables and their
  declarations, metadata, comments, separators and status lines are coloured,
  and the JSON, XML, GraphQL and JavaScript parts are coloured by their
  injected languages.
- `crates/fastedit/tests/grammar_pack_recorded.rs`: add a `NOT_RELEASED` row
  for `http` in alphabetical order (after `heex`), "added after grammars-2;
  recorded by the Mac's next grammar release". Don't edit `pack.toml`.
- Attribution: append one `http` row to `THIRD_PARTY.md`'s "Grammars shipped
  as WebAssembly" table, after the last row, in the `nim`/`verilog` rows'
  format: repository and full commit, and a licence cell naming the
  grammar's licence and the source of `highlights.scm` and `injections.scm`
  (upstream's `queries/kulala_http/` at that commit, MIT, adapted as their
  headers say). That row plus the queries' headers is the whole attribution
  (the T1463 convention): do NOT add a row to "Code copied or adapted into
  this repository" (`crates/` and `kiln/` only; an `extensions/` row fails
  `third_party::copied_files_and_rows_agree`) and do NOT touch `NOTICE`
  (`notice::sources_are_in_notice`). Reviewers: a missing copied-code row or
  NOTICE entry is this convention, not an omission. Append only
  (`shared_append_only`).
- Change nothing else. If the module won't load (ABI, the table validator, a
  libc import, over `MAX_GRAMMAR_BYTES` = 16 MiB, the load deadline), or a
  built-in already claims `.http` or `.rest`, block the card with the exact
  error: fixing fe-syntax, the vendored tree-sitter or the build script is
  out of scope.

## Implementation notes

- Pinned at main HEAD `1e238f6d…` (still the newest commit). The kulala grammar builds (323 KB wasm, ABI 15), loads, and the sample parses with no ERROR or MISSING node; rest-nvim's grammar wasn't needed.
- `crates/fastedit/tests/grammar_pack_recorded.rs` and its `NOT_RELEASED` list were retired on main by T1486 (997666b7), before this lane's base; there is nothing to append, so that part of C1 and the `grammar_pack_recorded` part of C5 are void (as on the batch card, T340). `pack.toml` is gone too; NOTICE untouched.
- Captures mapped as the `highlights.scm` header lists. The `#` of a `# @name` line and the `>>`/`>>!` of a redirect are anonymous regex tokens that queries can't name, so the whole metadata line is `@comment` under its own captures and the whole redirect `@punctuation.special` under its path (that span also covers the redirect's trailing newlines, which show nothing).
- Quoted JSON/GraphQL strings are single tokens upstream, so a `{{var}}` inside `"..."` reads as the injected language's string; outside quotes (headers, URLs, form bodies) variables are the host's `@variable`, `{{$name}}` dynamic ones `@variable.builtin`.
