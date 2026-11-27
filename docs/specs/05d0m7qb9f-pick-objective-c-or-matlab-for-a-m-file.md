---
task_uid: "01m8cgbh7tx56j5d05d0m7qb9f"
task: "T2136"
rev: 7
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "fe_syntax::detect_language picks objc for a .m file with Objective-C markers (#import, @interface, @implementation, @end) and matlab for one with MATLAB markers (function, classdef, % comments, clc/clear), when both are registered, and objc when it cannot tell (objc wins the .m tie, user decision 11-26); tested in crates/fe-syntax/tests/m_file_sniff.rs. Known limits of this line-level sniff, left to T2176 (lead decision on review sapy13): telling a MATLAB % comment from C modulo when a ; separates the % from a /* or a C comment ends in */; (F5 remainder), and single quotes inside a double-quoted C string (F7), and a C character literal inside fprintf's first argument, the stream expression, as in fprintf(c == '%' ? stdout : stderr, \"%c\", c) or fprintf(streams['%'], \"%d\", n) (F11, review cj28h4), and a C escaped-apostrophe character literal ('\\'') in a later fprintf argument read as a MATLAB doubled quote, so the scan runs past the call into a trailing comment with %, as in fprintf(stderr, \"%c\", '\\''); // 100% (F12, review bgkncv), and MATLAB's command form of clear with options (clear -regexp ^tmp; as the only MATLAB marker), which falls back to objc (F13, review 46ae03)"
    required: true
    manual: false
  - id: "C2"
    text: "A modeline still beats the sniff, and no extension other than .m, no other pair than objc and matlab, is affected (tests name .mm, .mat, .mlx and a third claimant of m)"
    required: true
    manual: false
  - id: "C3"
    text: "wasm_parity's every_extension_is_well_formed lets objc and matlab both claim m and still fails for any other duplicate claim, for objc or matlab with a third claimant, and for any other extension"
    required: true
    manual: false
  - id: "C4"
    text: "The grammar install offer for a .m file with neither grammar installed offers objc, not matlab: crates/fastedit/src/grammar_registry.rs pick no longer settles this tie by the lower name (objc wins the .m tie, user decision 11-26), with a unit test there; other ties keep today's behaviour"
    required: true
    manual: false
  - id: "C5"
    text: "cargo nextest run -p fe-syntax passes (the whole crate), and the grammar_registry tests in the fastedit crate pass"
    required: true
    manual: false
---

# Pick Objective-C or MATLAB for a .m file by its content in fe-syntax, and let both grammars claim m

# Pick Objective-C or MATLAB for a `.m` file by its content

`.m` is Objective-C's (`extensions/grammars/objc/extension.toml`: `extensions = ["m"]`) and MATLAB's only real extension, so the MATLAB grammar (T2135) can't ship until the editor can tell the two apart by content. The abandoned T1916 had this idea as part of a larger card; this one is only `.m`. It adds no grammar and changes nothing about any other extension.

## Today

- `fe_syntax::detect_language(path, text_start, text_end)` (`crates/fe-syntax/src/detect.rs`) runs: modeline, then the path (`language_name_for_path`, in `crates/fe-syntax/src/language.rs`, which asks built-ins and then registered grammars through `runtime::name_for_file`, first registered wins), then shebang, first line, and `language_name_by_content` (`crates/fe-syntax/src/sniff.rs`, XML and JSON only, and only for paths no language names).
- Callers: `crates/fe-gpui/src/pane_item.rs` (`EditorView::detect_language`) and `crates/fe-gpui/src/diff_view.rs` (`detect_language_name`); plus `crates/fastedit/src/grammar_install.rs` through the view after an install.
- `crates/fe-syntax/tests/wasm_parity.rs` `every_extension_is_well_formed` fails when two directories claim one extension ("are each for *.m", lines ~1002-1010 at main), so MATLAB can't list `m` today.
- The install offer (`crates/fastedit/src/grammar_registry.rs`, `pick`) takes the lower language name on a tie and logs a warning: with both claiming `m`, a `.m` file with neither grammar installed would be offered `matlab` before `objc`. Don't change the offer here; state in the finish note what it does and the lead decides.

## Do

1. In `crates/fe-syntax/src/sniff.rs`, add `pub(crate) fn language_name_for_m_file(text_start: &str) -> Option<&'static str>`-style logic (name it as fits) that, for a `.m` file, picks `objc` or `matlab` from the text:
   - Objective-C when any of these appears at the start of a line, past leading whitespace and comments: `#import`, `#include`, `#define`, `@interface`, `@implementation`, `@protocol`, `@class`, `@end`, `@property`, `@synthesize`, `@autoreleasepool`, `@import`, or a `//` or `/*` comment before any MATLAB marker.
   - MATLAB when none of those appear and any of these does at the start of a line: `function` (followed by a name, `[`, or `=`) , `classdef`, `%` or `%{` (a comment), `clc`, `clear`, `close all`, `format long`, `end` closing a block alongside `function`/`if`/`for`, or `disp(`/`fprintf(` with a `%` format.
   - Otherwise (empty, or unsure) the language the path alone gives today, which is `objc`: no behaviour change when it can't tell. Read only `text_start` as callers pass it (a few hundred bytes); do not widen the read.
2. In `crates/fe-syntax/src/detect.rs` `detect_language`, after the path step, when the path's extension is `m` (ASCII case-insensitively, a plain `.m` not a compound) and the path chose `objc`, or `matlab` when it is registered, run the sniff and use its answer if that language is compiled in or registered; if it picks `matlab` and none is registered, or `objc` and none is registered, keep the path's answer (never fall to plain text when the path had an owner). A modeline still beats it. Other extensions are untouched. If `language_name_for_path` itself (used elsewhere, e.g. fe-nav) is left returning the registered owner for `.m`, that is fine: say which callers still use it in the finish note.
3. In `crates/fe-syntax/tests/wasm_parity.rs`, let `m` be claimed by `objc` and `matlab` together: a named constant (`SNIFFED_EXTENSIONS`, `("m", ["objc", "matlab"])`) that the duplicate-extension check at ~lines 987-1010 skips for exactly that pair, with a comment pointing at the sniff. No other pair or extension is exempt, and `objc` alone, `matlab` alone or a third language claiming `m` still fails.
4. Tests, in a NEW file `crates/fe-syntax/tests/m_file_sniff.rs` (the sniff's decisions on real-looking files, both ways, the unsure default, a modeline winning, `.M`, `.mm`/`.mat`/`.mlx` untouched) and in the same file the exemption's two sides if `wasm_parity.rs`'s check is reachable as a function; else cover it in `wasm_parity.rs` itself. For the detect wiring, register stand-in languages through the existing helpers in `crates/fe-syntax/tests/runtime_grammar_comments.rs` (or a small built wasm if one is already available): the real `matlab` wasm does not exist until T2135, so the test must not depend on it.

## Scope and requirements

- Change only `crates/fe-syntax/src/sniff.rs`, `crates/fe-syntax/src/detect.rs`, `crates/fe-syntax/tests/wasm_parity.rs`, the new `crates/fe-syntax/tests/m_file_sniff.rs` and, if its `use`/re-export list must grow, `crates/fe-syntax/src/lib.rs` (shared append-only). No `extensions/grammars/**` change: objc keeps `m`, and T2135 then adds `m` to MATLAB's manifest.
- No new setting, no picker row, no remembered choice (T1916's other halves stay abandoned).
- Don't claim `.cls`, `.pl`, `.v` or any other contested extension here.
- Run `cargo nextest run -p fe-syntax` (the crate's whole suite) before finishing.

## Implementation notes

- The sniff is `sniff::language_name_for_m_file`, reading the first 512 bytes (`SNIFF_BYTES`, as the XML/JSON sniff does). Text inside `/* */` and `%{ %}` block comments doesn't count. MATLAB's `fprintf(`/`disp(` marker needs a single-quoted string with a `%` in it, so C's `fprintf(stderr, "%d", x)` stays unsure. `clear`/`clc` count only as commands (`clear(x)`, `clc = 2` don't).
- `detect_language` runs it (`detect::m_file_language`) only when the path's extension is `m` in any ASCII case and the path gave `objc` or `matlab`. When it can't tell it takes `objc` if loaded, so with both registered `objc` wins regardless of which registered first (C1). A pick that isn't loaded keeps the path's answer.
- `language_name_for_path` still returns the registered owner for `.m` (first registered wins). Its callers outside fe-syntax: fe-nav (`extract.rs`, `references.rs`, `scip_provider.rs`), fe-structural `scan.rs`, and the install offer in `crates/fastedit/src/grammar_registry.rs` (`language_for`/`language_to_offer`). So with objc installed and matlab not, a MATLAB `.m` file gets no install offer for matlab, because the path is already claimed.
- C4: `grammar_registry::pick` settles a tie via `TIE_WINNERS` (`("m", ["objc", "matlab"])`): the first one wins when the tied languages are exactly that pair and the file has the extension. Any other tie, a third `.m` claimant's included, still goes to the lowest name. The tests are in `grammar_registry::tie_tests`.
- Review round 1 (codex e0tcz2) changes:
  - F1: block openers may be followed by `(`, and `function` by `[` with no space (`if(x)`, `function[y] = f`).
  - F2: `%{ %}` comments nest.
  - F3: a C `/*` opened after code, outside `"`/`'` strings and before `//`, hides the lines up to its `*/`. A `%` ends that scan once the text looks like MATLAB, or when it follows a `;` or `,`, where C's modulo can't be.
  - F4: a double-quoted format counts when it is the first argument, or the first in a leading `sprintf(`. A single-quoted one with a `%` counts anywhere, so `fprintf(fid, '%s', x)` is MATLAB and C's `fprintf(stderr, "%d", x)` is not.
- Review round 2 (codex etw7wc) changes:
  - F1: `function` followed by `=` now counts, as the spec lists it (`function = f(x)`, `function=f`).
  - F5: a `%` outside strings ends the C-comment scan unless a `;` follows it before the `/*`, as in C's `a % b; /*`. So `x = 1 % see /* this` opens no comment.
  - F6: double-quoted formats are read whole, with `""` as an embedded quote. Single-quoted `''` already worked and is now tested.
- Review r4b8rk round 1 (after the rev 3 respec) changes:
  - F8: a single-quoted format counts only inside the call. The scan ends at the call's `)` or at a comment outside a string, skips double-quoted strings, and ignores C character literals (`'%'`, `'\%'`).
  - F9: the sniff is skipped when a registered compound extension ending in `.m` claims the file (`runtime::compound_owners`), so `a.special.m` keeps its owner. The install offer's `TIE_WINNERS` applies only to extension picks (a `by_extension` flag on `pick`), and only when plain `m` is the longest claim each tied language has on the file.
  - F5 (remainder) is a known limit in C1, left to T2176. It is not changed here.
- Review qhcdwf round 2 changes:
  - F10: the C character-literal exclusion now applies only to `fprintf(` and only after its first argument (C's first argument is a stream). A literal is exactly one character, or a `\` and one. So `fprintf('\n%d\n', 3)`, `fprintf(fid, '\n%d', x)` and `disp('%')` are MATLAB again, and F8's C cases stay excluded.

