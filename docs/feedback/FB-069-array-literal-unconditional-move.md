# FB-069 — genArrayLiteral 非 Copy 臂无条件 emitMove，与 genTupleLiteral 记账不一致

genArrayLiteral 的非 Copy 臂无条件 emitMove（22180 区），而同形 genTupleLiteral 臂先 markStoredValueMovedIfNeeded 且仅对非局部寄存器 emitMove——同族两函数的存值所有权记账不一致：局部寄存器存入数组被 move_ 退休、存入元组仅标记消费，SAB 验证器可见。

状态：四期 SAB-I 线登记的候选（来源：p4-deliver/sabi/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认后入 Zig 修复队列。
