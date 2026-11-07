# FB-047 — unterminated `[MACRO]` block silently registers, body runs to EOF

Status: candidate, venue-probed at box head 2cc12ea8.
Evidence: docs/probes/imp/probes5.out (unterm_macro).

## Input

```sa
[MACRO] LOOSE %out, %val
    %out = copy %val
```
(no `[END_MACRO]` anywhere; the importing program calls
`LOOSE(a, 5)`)

## Observed

`sa sla check` rc=0. `loadImportedMacrosFromExpandedSource`
(plugin_imported_macros.zig, box head) reads body lines until
`[END_MACRO]` OR end of input, then registers the macro exactly
as if the block had been closed.

## Why it matters

Any content after an unterminated header — including what the
author intended as subsequent macros or code — is silently
swallowed into the macro body, and the macro is callable. A
missing `[END_MACRO]` is precisely the kind of typo a loader
should reject loudly (the SA assembler itself requires the
terminator). Rust side mirrors the EOF-terminates semantics
verbatim and pins it in its loader tests' shapes.
