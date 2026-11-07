# FB-048 — macro index cache registers a partial macro set from a corrupt cache

Status: candidate, source-cited (observation level; the cache
path is a Wyhash digest, so the corrupt file cannot be crafted
from outside without reproducing the key).

## Mechanism

`tryLoadImportedMacrosFromCache` (plugin_imported_macros.zig,
box head) parses `.sla-cache/macros/*.idx` records field by
field. For arity / leading_outputs / borrowed / address_slot it
does `parseInt(...) catch continue` — an unparseable field skips
THAT macro silently while the remaining records of the same cache
file still register, and the function returns `true` (cache
accepted). Only a MISSING assigned-output field (pre-mask cache
records) returns `false` and forces a full rescan.

## Why it matters

The two policies are inconsistent: one class of cache corruption
produces a complete rescan, the other produces a silently
incomplete macro registry (the skipped macro's calls then fail
downstream as unknown, far from the cause). A corrupt cache is
exactly the input a cache loader should distrust wholesale.
Suggested shape: any parse failure in a record invalidates the
whole cache file (return false), matching the assigned-field
policy.
