# A+K2 设计条目（五期预备登记 · 非本批实现门）

来源裁定（2026-11-07，用户）：
- 17:29 A 原则批准（Rust `unsafe` 类比：命名、窄域、可审计的逃逸口是安全模型的正规组成部分）。
- consult-ffi-unsafe 裁断 K2（六家到场四家全向，Opus 概率票 K2 0.50 居首）：资格附着于发布事件、SLA 前端 `unsafe {}` 辖区执法、宿主认资格发射形、认形状仅作非豁免顾问、无双钥过渡。
- 17:44 用户：「RUST 也按照这种做」——本条目即该裁定在本仓的登记；PHASE4_STATUS.md 末节存其原文三条。

## 规则（两实现同规，本仓实现时逐条对照）

1. 经发布面暴露给 FFI 的函数对象 cell，仅豁免 exit 泄漏检查；UAF / double-free / provenance 规则不变；同形未发布仍 trap；发布后 double-free 仍 trap。
2. 资格附着于发布事件：发布须写在既有 `unsafe {}` 辖区内才获得资格发射形；前端执法（检查器追踪 unsafe 上下文至发布点），宿主/裁判认资格形；块外发布保持可调用、继续响亮报错（1012 不豁免）；块内普通分配不沾光。
3. 认形状仅许作非豁免顾问（提示迁移、不据此豁免）；无双钥过渡。

## 载体合同对接点（等 Zig A 线 RESULT 定版）

对接线：def-sci-fnptr-escape（Zig 侧实施中）。本条目登记其 WIP 已现的载体方向（2026-11-07 晚协调线只读所见，**以其 RESULT 为准、此处不作定版**）：
- 资格发射形 = 专用展开名 `PTR_ADDR_PUBLISH`（[MACRO] 包装，内层仍走 PTR_ADDR 发布语义）；普通 `PTR_ADDR` 永不打标。
- 宿主侧（sci referee）以 `published_escape` 登记被 PUBLISH 发布的寄存器，仅在 exit 泄漏检查处豁免。
- SLA 语料已现三形：`test_unit_fnptr_unsafe_publish_{expr,stmt,spawn}.sla`（表达式/语句/thread spawn 发布面）。

本仓实现子线开启条件（满足才派，不在本批内）：
- Zig A 线 RESULT 交付，且载体合同定版（资格形在 SA 文本与 SAB 中的表示、SAB 编码是否原样携带展开名、前端执法点的精确集合）。
- 正负语料对落地 Zig 主线（块内发布豁免+可调用 / 块外发布续报 1012 / 同形未发布照 1012 / 发布后 double-free 照 trap）。

## 本仓预定镜像面（实现子线辖区草图，开启时以 RESULT 复核）

- sla-parser / sla-checker：unsafe 上下文追踪至发布点（发布点集合 = RESULT 定版：取函数地址、向 extern/线程面传递函数值的位点）。
- sla-codegen-text / sla-codegen-sab：仅在 unsafe 辖区内发射 `PTR_ADDR_PUBLISH` 资格形；块外发射普通形。SAB 路须证资格形经 SAB 编码/解码不丢（与宿主 `sa` 装配端到端：build-exe/test 下豁免生效）。
- sla-cli：无独立面（经两 codegen 与委派接缝自然到达）；端到端验收以 Zig 侧语料对在本仓差分对拍（同规同语料口径）。
- 登记联动：实现落地时在 docs/divergences/cli-commands.md 与 sab 台账各记一行指向本条目；sala 口径由主会话随 Zig 批一处落字、两实现共用（本仓不单独改 sala）。
