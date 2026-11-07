# FB-036 — handler_bridge 的 import 展开把 IO/parse 失败统一吞成 InvalidHandlerOutput

- **反馈环批次**：第四批（三期 HND 评估线发现、协调线收编）
- **源登记**：`docs/phase3/handler-eval.md` §6 HND-C2
- **分类**：Zig 侧缺陷候选（小，诊断保真）—— 源码机制定案
  （静默吞错族）；随 FB-035 的面 B 去留裁定一并处置
- **Rust 侧处置**：面 B 未接线，无镜像对象；仅登记。

## 机制（Zig 源码位置，本地参照树行号；handler 面已经场地抽查对版）

- `src/handler_bridge.zig:195-198` 的 catch 链：import 展开路径
  中「读文件失败 / expand 失败 / parse 失败」三类不同错误被统一
  吞成 `InvalidHandlerOutput`。
- 宿主侧以 error_name 读得的诊断与真实因（IO 错误 / 语法错误）
  脱节；在 rc=2 分类下会误导排障方向。

## 建议修复形态（仅建议）

保留原始错误类别（或至少在结果中附带原始 error 名），使
InvalidHandlerOutput 只用于真正的输出形态错误；修复后复核
口径：文件缺失与语法错误两输入给出可区分的诊断。
