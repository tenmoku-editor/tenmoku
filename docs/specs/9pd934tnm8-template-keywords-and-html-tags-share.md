---
task_uid: "01m8cd9p34whygr89pd934tnm8"
task: "T2114"
rev: 1
type: "bugfix"
complexity: "s"
criteria:
  - id: "C1"
    text: "In Yohen, Hakuji and Tenmoku Ember, syntax_color(\"tag\") differs from syntax_color(\"keyword\") and from syntax_color(\"punctuation.special\"): fe-config test tags_stand_apart_from_keywords_and_template_delimiters in crates/fe-config/tests/editor_palette.rs passes (and fails at the base)"
    required: true
    manual: false
  - id: "C2"
    text: "Yohen tag = #7fc9be, Hakuji tag = #0d6e66, Tenmoku Ember tag = #d9b36c; keyword and punctuation.special colours are unchanged in all three"
    required: true
    manual: false
  - id: "C3"
    text: "docs/design/design-system.md Syntax table has a Tag row (Seiji, #7fc9be / #0d6e66)"
    required: true
    manual: false
  - id: "C4"
    text: "cargo nextest run -p fe-config passes; no screenshot golden changes"
    required: true
    manual: false
---

# Template keywords and HTML tags share one colour in Yohen, Hakuji and Ember: give tag its own glaze (Seiji)

## Problem (USER report, 11-26, screenshots of extensions/grammars/handlebars/test/sample.hbs and jinja/test/sample.jinja)

In template languages (Handlebars, Jinja, and likewise Twig, Liquid, Blade, ERB, EJS, Go templates), the template
syntax and the HTML around it come out the same colour, so you can't tell `{% if %}` / `{{#each}}` from `<li>` / `<span>`.

Cause: the house themes give `tag` the keyword colour, and `punctuation.special` (the `{{ }}` / `{% %}` delimiters) too:

| Theme | keyword | tag | punctuation.special |
| --- | --- | --- | --- |
| Yohen (default dark) | `#c7a5e6` | `#c7a5e6` | `#c7a5e6` |
| Hakuji (default light) | `#7b36a8` | `#7b36a8` | `#7b36a8` |
| Tenmoku Ember | `#d9694a` | `#d9694a` | `#c9a98a` |

The grammars are fine: Jinja/Handlebars capture `@keyword*` and `@punctuation.special`; HTML captures `@tag`.

## Fix (lead's design call)

Give tags their own glaze in the house themes; the template syntax keeps the keyword colour (delimiters with it), so
template = jun, markup = a different hue, attributes stay Lapis.

- Yohen `tag` = Seiji `#7fc9be`; Hakuji `tag` = Seiji `#0d6e66` (the Type glaze: no types in markup, and it stands
  apart from Jun keywords and Lapis attributes in both themes).
- Tenmoku Ember `tag` = its type colour `#d9b36c`.
- Add a **Tag** row to the Syntax table in docs/design/design-system.md (Seiji, "HTML/XML/JSX element names").
- Leave the faithful ports alone: Monokai and VS Code Dark Modern use one colour for tags and keywords in their
  originals too. Say so in the decision file as a known limit.

If a test pins the old tag colours, update it; if that test is outside scope, respec to add it.
No screenshot scene shows markup today, so no golden should move. If one does, stop and report: goldens are 2x and
the Mac builds nothing now.

## Implementation notes

- Carried finding T1849 F14 (the contrast test omits hover over the cursor line) was already fixed at this base by
  T1881: `every_bundled_themes_glyphs_keep_the_contrast_floor` in `crates/fe-config/tests/debug_tints.rs` has the
  "hover tile on the cursor line" ground. This change touches no debugger colour.
- `yohen_hakuji_values.rs` maps the design's new Tag row to `syntax.tag`.
