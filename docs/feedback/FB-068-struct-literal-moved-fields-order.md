# FB-068 — genStructLiteral 的 pending_moved_fields 以哈希序发射

genStructLiteral 的 pending_moved_fields 以 AutoHashMap 哈希序发射 move_ 指令——多字段 move 的结构体字面量 SAB 产物字节序不确定（FB-016/FB-020 同族）。

状态：四期 SAB-I 线登记的候选（来源：p4-deliver/sabi/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认后入 Zig 修复队列（与主线 det 确定性修复同族）。
