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

## 五期现头复核（2026-11-07，FB 确认批 1）

场地同 FB-025 复核（Zig = sla 现头 08fabac6 pristine 克隆装机；
Rust = sla-cli @ 65a642e+本批修复）。

- **Zig 现头两表现均仍在，本单对 Zig 侧保持开放**：
  表现 A：`sa sla check` / `build` str_a 均 rc=1，
  `Macro Expansion Error: failed to expand tuple templates in
  str_a.sla: error.InvalidExpandTuple`。
  表现 B：str_b check/build rc=0，产物 .sa 中字符串为
  `@const SLA_STR_0 = utf8:" not a directive ` ——仍被静默改写。
- **Rust 侧：根文件路不经 expand，但函数级缺陷在、经导入模块
  可达，已修**：
  - sla-cli 根文件管线直接解析原文（不跑 source_expand），故
    str_a/str_b 经 `sla check`/`build` 均 rc=0、字符串逐字完好
    （产物 `@expand_tuple(x)\0` / 完整字面量）——根文件形态不复现。
  - 但 `sla-parser/src/source_expand.rs::expand` 逐字镜像了无
    词法感知扫描，经 `@import` 的 .sla 模块（contract load /
    sla_expand / imported_macros 三处调用）可达：修复前，根文件
    `@import "probemod_a.sla"`（模块内含表现 A 字符串）构建
    rc=1 `Import Error: failed to load @import contracts:
    error.InvalidExpandTuple (in probemod_a.sla)`，端到端复现。
  - 修复：`expand` 顶层扫描跳过字符串字面量（含转义引号）与
    `//` 注释，逐字拷贝——采建议形态前半。单元测试（字符串/
    注释/转义/真指令后置四组）修复前失败、修复后通过；修复后
    main_a 的失败形态与良性字符串对照组 main_c 完全一致
    （下游「根导入不合入模块 fn」为既有形态限制，与本单正交）。
  - Rust 侧就本单销账；Zig 侧候主线按建议形态修复。
