# FB-035 — 出货 libsla.so 未导出第二 ABI（sla_compile_handler），宿主内亦无消费者

- **反馈环批次**：第四批（三期 HND 评估线发现、协调线收编复核）
- **源登记**：`docs/phase3/handler-eval.md` §6 HND-C1
- **分类**：Zig 侧缺陷候选 —— 场地实测三手法互证 + 协调线独立复核；
  「缺陷还是废弃面」候主线裁定（见下两读）
- **Rust 侧处置**：sla-plugin 保持不导出（与出货参照符号面对齐），
  本单登记；不先行导出以免符号面多于参照。

## 现象（sla 现头 6002b386，Kaggle 场地三实测）

- 已装插件与 zig-out 两份 `libsla.so` 的 `nm -D --defined-only`
  均只有 2 个符号：`saasm_plugin_descriptor_v1`（D）与
  `saasm_plugin_descriptor_v1_fn`（T）。协调线另行直查 zig-out
  一份复核一致。
- `strings` 中 `sla_compile_handler` 出现次数为 0；ctypes dlopen
  查 `sla_compile_handler` / `sla_compile_handler_result_free`
  均为 False（Rust 侧 release .so 同为 False——两边符号面当前
  恰好一致）。
- `grep sla_compile_handler sci/src/` 零命中，`sa` 二进制 strings
  亦为 0：**sci 宿主内没有该符号的任何消费者**。
- plugin_handler.zig 在 sla 仓 git 史只有一笔 `e68634c6 part1`，
  唯二消费者是其自带单测。

## 与自家文档的矛盾

本线 `docs/ABI_MAP.md` §1 把该两符号列为「libsla.so 实测 nm -D」
的 T 符号——与现头实测不符。ABI_MAP 已加更正注（2026-11-07）。
成因未定，三读并存：① 写作时参照的本地树（3de91932，2026-10-26）
确曾导出、其后丢失；② 当时测量有误；③ 两者皆非、构建形态差异。
候主线以 git 史 + 构建产物裁定。

## 候选机制（未证推断）

plugin.zig:25-26 仅以 `pub const` 再出 plugin_handler 的导出项，
lib 根无其他引用，Zig 惰性分析可能从未发射该 export。行号据本地
参照树，handler 面已经场地抽查对版。

## 两读（候主线裁定）

- **读 A（缺陷）**：handler 是预期中的宿主 ABI（源码在、单测在、
  ABI 文档在），导出静默丢失 = 宿主能力面实际不存在，属静默
  兜底族的构建面变体。
- **读 B（废弃）**：该面已被描述符 handle_command 取代而无人
  消费，handler 代码与 ABI_MAP §5 应一并退役/更正。

## 建议修复形态（仅建议，随裁定二选一）

读 A：使 export 真实发射（根文件显式引用或 comptime 锚），并以
出货 .so 的 nm -D 清单进金丝雀类常设门（符号面回归从此有门）。
读 B：删/标退役 handler 面与文档段，消除「文档声称存在、产物
不存在」的半死状态。
