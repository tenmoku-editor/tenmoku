# Working in this repo as an agent

CONTRIBUTING.md covers setup, tests and code style. This file adds what agents
get wrong most often.

## Reuse before you write

Agent-written code drifts toward fresh copies: a second spinner, another popup
list, one more temp-project test helper. Each copy then drifts from the others
in look, behaviour and bugs. Before you write a widget, helper, test fixture or
utility:

1. Search for an existing one. `rg` the kiln crates (kiln-ui widgets,
   kiln-shell chrome, kiln-core app plumbing, kiln-plugin) and the fe-* crates
   for the concept, not only the name you'd give it: a spinner is also
   `LoaderCircle`, `rotate` or an animation ticker.
2. If one fits, use it. If it nearly fits, extend it with a parameter, a
   variant or a builder method rather than writing a parallel version.
3. If you are about to make a second copy of something that exists, factor the
   shared part out instead. UI primitives go in kiln-ui, app plumbing in
   kiln-core, editor-only helpers in the lowest fe-* crate both users depend on,
   and test helpers in that crate's test support module.
4. If the shared home is outside your task's scope, don't copy around it. Use
   what exists as it is if you can, and if reuse needs a small scope addition,
   block and ask (`rgt task block --note "<why>"`). Otherwise record in the
   spec's `## Implementation notes` what should be shared, with file:line for
   each copy, so the lead can file a follow-up.
5. In `## Implementation notes`, name the existing components you reused or
   extended.

## Reviewing

A reviewer flags new code that duplicates an existing component: a parallel
implementation of something the repo already has (a widget, helper, test
fixture or utility doing the same job), where using or extending the existing
one was possible. The finding names the existing component with file:line
and says how the new code could use or extend it. Code that only looks alike
but does a different job is not a duplicate. When the shared home is outside
the task's scope and the worker recorded the copies in Implementation notes,
say so in the finding, so the lead can make it a follow-up instead of another
round.
