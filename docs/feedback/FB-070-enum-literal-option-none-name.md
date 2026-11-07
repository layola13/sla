# FB-070 — genEnumLiteral 以名字字符串特判 `Option::None`

genEnumLiteral 以名字字符串特判 `Option::None`（21478）——用户自有名为 Option 的枚举带 None 变体时会被劫持到 std 片段路径（INT2-1 名字特判家族）。

状态：四期 SAB-I 线登记的候选（来源：p4-deliver/sabi/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认后入 Zig 修复队列。
