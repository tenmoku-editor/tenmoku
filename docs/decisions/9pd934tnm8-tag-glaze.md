# Tags get their own glaze in the house themes (T2114)

In template languages (Handlebars, Jinja, Twig, Liquid, Blade, ERB, EJS, Go
templates) the template's keywords and delimiters came out the same colour as
the HTML around them: Yohen, Hakuji and Tenmoku Ember gave `tag` the keyword's
colour.

## Ruling (lead)

`tag` moves to another hue; `keyword` and `punctuation.special` stay, so the
template syntax keeps the keyword's colour and attributes keep their own.

| Theme | `tag` | Was |
| --- | --- | --- |
| Yohen | `#7fc9be` Seiji | `#c7a5e6` jun |
| Hakuji | `#0d6e66` Seiji | `#7b36a8` jun |
| Tenmoku Ember | `#d9b36c`, its type colour | `#d9694a` |

Seiji is the Type glaze: markup has no types. The colour applies wherever a
grammar captures `@tag`, not only in templates.

## Known limit

Monokai and VS Code Dark Modern still draw tags and keywords in one colour, as
their originals do. They are faithful ports, so a template's keywords and its
HTML tags look the same in them.
