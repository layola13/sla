# FB-046 — workspace manifest diagnostics: silent ignores and an unlocated bare error

Status: candidate, source-cited + one venue probe at box head
2cc12ea8. Evidence: docs/probes/imp/probes5.out (ws_typo_root).

## Mechanism (two halves of one surface)

1. `workspace.parseManifestWithFile` silently IGNORES any
   top-level line that is neither `package` nor `workspace`
   (no else error): a mistyped or stale key at the top level of
   sa.mod degrades the manifest without a word.
2. When a manifest error DOES surface at the command layer, it is
   context-free. Venue probe: a workspace root whose sa.mod has
   the header typo'd as `workspac {` (with members/default_member
   lines following) — `sa sla check` at the root fails rc=1 with
   the entire diagnostic being:
   `error: InvalidFormat`
   No file, no line, no key named.

## Why it matters

sa.mod is the entry point of every workspace-shaped compile; the
two halves together mean manifest mistakes are either invisible
(half 1) or unactionable (half 2). Same silent-swallow family as
D-C2. Rust side mirrors both halves verbatim (parse errors carry
no span in Zig's ResolveError either).
