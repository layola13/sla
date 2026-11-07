# FB-067 — genUnsafeExpr void 形的零哨兵仅在 fall-through 路径赋值

genUnsafeExpr void 形的零哨兵仅在 fall-through 路径赋值（13484–13490，Zig 注释自承）：终止路径上哨兵寄存器未定义即被返回，与 FB-065 同族（簿记 tmp 未定义族）。Rust 侧逐字镜像。

状态：四期 SAB-H 线登记的候选（来源：p4-deliver/sabh/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认后入 Zig 修复队列。
