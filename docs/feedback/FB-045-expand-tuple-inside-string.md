# FB-045 — source_expand expands `@expand_tuple` inside string literals (and rejects valid strings)

Status: candidate, venue-probed at box head 2cc12ea8 (installed
plugin, `sa sla check` / `sa sla build`, SLA_NO_FRESHNESS_CACHE=1).
Evidence: docs/probes/imp/probes4.out + build artifact inspection.

## Mechanism

`source_expand.expand` (source_expand.zig, box head) scans the raw
source bytes for the substring `@expand_tuple` with no lexical
awareness: string literals and comments are not skipped at the
top level (brace/paren matching skips strings, but the SCAN does
not). A directive-shaped text inside a string literal is treated
as a real directive.

## Manifestation A — valid program rejected

```sla
fn main() -> i32 { let s = "@expand_tuple(x)"; return 0; }
```
`sa sla check` -> rc=1:
`Macro Expansion Error: failed to expand tuple templates in bad.sla: error.InvalidExpandTuple`
The string is a perfectly valid literal; the expansion layer
rejects the whole compilation for text it should never have read
as a directive.

## Manifestation B — string value silently rewritten

```sla
fn main() -> i32 { let s = "@expand_tuple(1, 1, T) { not a directive }"; return 0; }
```
check rc=0, build rc=0 — and the emitted `main.sa` contains the
string ` not a directive ` (grep confirms: the directive text is
gone). The program's observable string value differs from what
the source says, with no diagnostic anywhere.

## Suggested shape (for the Zig-side fix line to裁定)

Make the top-level scan lexically aware (skip string literals and
`//` comments, the same helpers findMatchingBrace already uses),
or restrict directive recognition to line starts at brace depth 0
outside strings. Rust side mirrors the raw scan verbatim until
then (IMP-8); both manifestations are pinned by Rust unit tests
only in their Zig form (the expansion functions are byte-mirrored).
