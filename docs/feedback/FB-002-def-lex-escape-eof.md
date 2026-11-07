# FB-002 — lexer 转义扫描在 EOF 处越界一字节（invalid token 的 loc.end = len+1）

- **反馈环批次**：第一批（用户 2026-11-07 常设政策）
- **主线候选名**：DEF-LEX-ESCAPE-EOF（主会话已登记）
- **源登记**：`docs/divergences/lexer-ast.md` D-L2；合编 `docs/DIVERGENCES.md` K-1
- **分类**：Zig 侧缺陷候选 —— **已实证确认**（见下）
- **Rust 侧处置**：只镜像 + 登记。Rust lexer 两处均钳制到 `buffer.len`
  （tag/start 不变），该钳制是已裁决的已知有意分歧（K-1），不迁就回越界形态。

## 最小输入

5 字节：`"abc\`（双引号 + `abc` + 单个反斜杠，串未终结、反斜杠为尾字节）。
模板形态：`` `abc\ ``（反引号开头、同形尾字节）。

## 实证（原 Zig lexer，场地 Kaggle 2201，直接驱动 `lexer.zig`）

证据原文：`docs/probes/dl2_probe.out.txt`（探针 `docs/probes/dl2_main.zig`）。

| 输入 | buffer_len | invalid token loc | 判定 |
|---|---|---|---|
| `"abc\` | 5 | [0..**6**] | end = len+1 越界 |
| `"abc`（无反斜杠对照） | 4 | [0..4] | end = len 正常 |
| `"abc\\`（转义对对照） | 6 | [0..6] | end = len 正常 |
| `` `abc\ ``（模板） | 5 | [1..**6**] | end = len+1 越界 |
| `let s = "abc\`（语句形态） | 13 | [8..**14**] | end = len+1 越界 |

附带现象（模板路径）：越界后若消费者持续拉取 token，lexer 不再产出
eof，而是在 [6..6] 原地无限重复 invalid token（探针 32 个上限截断）。
停在首个 invalid 的消费者不受此影响；全量扫描型消费者（预扫描 /
高亮 / 错误恢复）会原地打转。

## 机制（Zig 源码位置）

- `lexer.zig` 字符串分支：转义跳过 `if (buffer[index] == '\\') index += 1;`
  叠加循环自身的递增，使尾字节为 `\` 时 index 走到 `buffer.len + 1`，
  invalid token 以此 index 收尾。
- `lexer.zig` `nextTemplate` 字面块：`if (c == '\\') index += 2;` 同形，
  可一步越过 len。

## 后果

任何按 `buffer[loc.start..loc.end]` 切片消费 loc 的下游都拿到越界区间：
Zig 安全构建下 panic，ReleaseFast 下为未定义切片。正常输入永不触发，
故 Zig 侧自查难以看见——这正是差分镜像照出的那类点。

## 建议修复形态（仅建议，不预判主线最终形态）

两处扫描循环的转义跳步钳制在 buffer 末尾（index 不越过 len），
保持 token 的 tag 与 start 不变；模板路径另需保证越界形态后能正常
产出 eof（或至少产出前进的 token），消除原地打转。

## Rust 侧单测锚点

lexer 线 D-L2 登记 + sla-lexer 既有测试集；修复后差分复核口径：
同上五例输入，Zig 侧 invalid token 的 loc.end 应全部 ≤ buffer.len。
