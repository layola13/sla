# FB-061 — `mergeRefCellCompanions` 按哈希图键序准备合并槽

Zig `mergeRefCellCompanions` 按分支 companions 哈希图键序准备合并槽——SAB 产物字节序随运行漂移候选（确定性家族，与 FB-016/020 同类）。

状态：四期 SAB-E 线登记的候选（来源：p4-deliver/sabe/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）；未做 Zig 侧复证，候主会话现头确认后入 Zig 修复队列。Rust 侧按寄存器升序发射（见 SAB-D101），不改行为迁就。
