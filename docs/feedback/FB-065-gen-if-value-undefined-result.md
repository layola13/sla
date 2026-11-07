# FB-065 — genIfValue 双终止臂的未定义 result 寄存器

genIfValue 双终止臂时 Zig 返回只登记未定义的簿记 tmp（同 SAB-D145 前半）：若表达式结果被消费，读到的是从未赋值的寄存器；Zig 靠「双臂皆终止则后续不可达」的上下文兜底，未在发射层响亮化。Rust 侧逐字镜像。

状态：四期 SAB-H 线登记的候选（来源：p4-deliver/sabh/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认后入 Zig 修复队列。
