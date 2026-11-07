# FB-037 — handler 编译写死 extern 前导三符号，表面限制未在 ABI 文档/类型面声明

- **反馈环批次**：第四批（三期 HND 评估线发现、协调线收编）
- **源登记**：`docs/phase3/handler-eval.md` §6 HND-C3
- **分类**：Zig 侧候选（观察级，表面限制未声明）—— 源码事实
  定案，未做行为实测；随 FB-035 的面 B 去留裁定一并处置
- **Rust 侧处置**：sla-abi 的 handler 类型面照现状不声明该限制，
  本单登记；不擅自增删声明。

## 事实（Zig 源码位置，本地参照树行号；已经场地抽查对版）

- `src/handler_bridge.zig:245-255`：`compileHandlerWithSupport`
  写死 extern 前导三符号（render / sax_get_time /
  sax_event_target_value_i64）。
- 后果：handler 源码调用任何其他宿主函数必在检查段失败——这
  是对 handler 作者的真实表面限制，但 `docs/ABI_MAP.md` §5 与
  handler 的类型面均未声明它，使用者只能撞墙得知。

## 建议修复形态（仅建议）

在 ABI 文档与 handler 声明面明示该三符号白名单（或改为可扩展
的前导声明机制）；复核口径：文档/类型面可查得限制原文。
