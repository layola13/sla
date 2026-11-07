# FB-026 — std 表面规则文件缺失时 SAB 静默以零规则继续（SAB-D02 家族新实例）

- **反馈环批次**：三期 SAB-D 线（D1 surface_load 移植中登记）
- **源登记**：`docs/divergences/sab.md` SAB-D28（SAB-D02 家族）
- **分类**：Zig 侧缺陷候选 —— 源码实证（场地三现头 sla
  6002b386 的 `sab_codegen.zig` 原文）；未单独构造运行实证，
  机制由代码直读确定。
- **Rust 侧处置**：控制流与零规则结果原样镜像，但每根候选
  的失败与最终落点写入 `SurfaceLoadLog`（SAB-D02 既定方针：
  响亮记录、不复刻静默）；不改变任何发射字节。

## 机制（Zig 源码位置，场地三现头行号）

- `src/sab_codegen.zig` `loadStdSurfaceRules`（1551–1554）：
  ```zig
  if (self.std_surface_rules.items.len != 0) return;
  const source = (try self.readStdSurfaceFile()) orelse return;
  ```
  `readStdSurfaceFile`（1273–1352）的四级回退链全部未命中时
  返回 `null`，此处即**静默返回、零规则继续生成**，无任何
  诊断、日志或指纹记录。
- 后果形态：此后一切 std 表面调用（`Vec::new`、`push`、
  `len(...)`、索引等）在 `findStdSurfaceRule` 处查无规则，
  统一以无 payload 的 `UnsupportedSabDirectFeature` 失败
  （SAB-D07：原因默认不可见）——现象与「该表面本就不支持」
  不可区分。规则文件随 sla_std 布局变动（checkout 挪位、
  SLA_STD_DIR 未设、烘焙目录过期）即整表蒸发，与主线已实证
  的「SAB 静默测到旧 std、绿证作废」同族（AGENTS.md
  2026-11-04 SAB 片段路由条）。
- 对照：同文件的 `resolveSaStdRoot`（1374–1428）在全败时
  `return error.FileNotFound` 响亮失败——同一 D 组内两条
  定位链的失败口径不一致，规则链是静默的那条。

## 建议形态

- `loadStdSurfaceRules` 在 `orelse` 分支至少留一条可观测
  记录（ tried 候选清单 + 最终零规则决定），口径与
  SAB-D02 对 `readStdSurfaceFile`/`resolveSaStdRoot` 吞错点
  的既有登记一致；或与 `resolveSaStdRoot` 对齐为响亮失败
  （二选一归主线裁决，本单只登记现象与机制）。
