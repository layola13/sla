# FB-008 — 构建产物中的 @test 为空壳：test 体在 codegen 前被流水线清空

- **反馈环批次**：第二批（lowering/text 线 (c) 类）
- **源登记**：`docs/divergences/lowering-text.md` D-T7；合编 `docs/DIVERGENCES.md`
- **分类**：Zig 侧缺陷候选 —— 流水线口径问题（非 codegen 本体错误）
- **Rust 侧处置**：gen_test_decl 同样忠实生成所给函数体；对拍台以
  空体喂入匹配 build 模式 golden。本单只陈述口径事实与后果。

## 机制（Zig 源码位置）

- `src/codegen.zig:7610` `genTestDecl` 忠实生成所给的函数体，
  本身无删减。
- golden（`sa sla build` 产物）里 @test 体为空，是
  plugin_compile 的构建模式在 codegen **之前**把 test 体清空
  所致——产物中的 @test 只是空壳。

## 影响

- 「产物即已测」的读法会高估实际执行过的断言数：查看 build
  产物的人会以为其中的 @test 携带了源测试的断言，实际为空。
- 口径风险在判读层：以 build 产物核查测试覆盖时得到虚假的
  完整感。@test 的真实执行面在 test 运行器路径，不在 build
  产物——两条路径的产物形态差异目前无显式标注。

## 建议修复形态（仅建议，不预判主线最终形态）

二选一并显式化：或在 build 产物中保留 test 体（与 test 路径
同形态），或在流水线/文档口径上明确标注「build 产物 @test
为空壳、覆盖判读以 test 运行器为准」，避免产物被误读为已测
证据。本单不断言哪种形态正确，只登记口径缺口。

## Rust 侧锚点

对拍台（sla-codegen-text 的 fixture_parity 与 sla-oracle）在
与 build 模式 golden 对拍时以空体喂入，注记见 COORDINATION
接线裁定③；主线口径定案后同步调整喂入形态。
