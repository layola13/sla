# FB-006 — println 对未知类型实参只打印 1 字节且无诊断

- **反馈环批次**：第二批（lowering/text 线 (c) 类）
- **源登记**：`docs/divergences/lowering-text.md` D-T5；合编 `docs/DIVERGENCES.md`
- **分类**：Zig 侧缺陷候选 —— 源码机制定案（静默兜底族）
- **Rust 侧处置**：逐字镜像（`crates/sla-codegen-text` println.rs 兜底臂），候主线裁决。

## 机制（Zig 源码位置）

- `src/codegen.zig` 约 2397-2417：`emitPrintln` 末尾兜底臂。
  `{}` 孔实参类型不在 int/float/bool/String/借用等已知形态内
  （或检查器未给该实参登记类型）时，发射
  `call @sa_print_bytes(&{ptr}, 1)` —— 恰好打印 **1 字节**，
  无任何诊断。

## 影响

- 格式化输出被静默截断为单字节：程序「正常运行」，输出错误
  且无信号，是最难定位的一类缺陷（用户只会看到输出少了）。
- 该兜底臂是差分线真实踩过的落点：对拍中一处实参节点 clone
  使地址键类型事实丢失，实参即落入此臂（见 D-T10）——说明
  「检查器未给类型」在真实管线中确实可达，并非理论分支。

## 建议修复形态（仅建议，不预判主线最终形态）

兜底臂改为响亮报错（实参类型未知/不支持 + 位置）；或在
emitPrintln 入口先断言每个 `{}` 实参都有已登记类型，未登记
即前端错误。修复后差分复核以「未登记类型的 println 实参」
为最小输入。

## Rust 侧锚点

`crates/sla-codegen-text` println.rs 兜底臂逐字镜像并注源行号；
D-T10 的地址键契约（喂入树不得 clone/搬动）已在协调线接线
中落实，避免 Rust 侧自身再造落入此臂的输入。
