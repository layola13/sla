# FB-066 — opKindForCast 对 float→unsigned 一律 fptosi

opKindForCast 对 float→unsigned 一律 fptosi（2428–2457 无 uitofp 对应臂）：负值/超界输入的语义与 SA 文本路是否一致未核，登记候选。Rust 侧逐条移植 opKindForCast，未改。

状态：四期 SAB-H 线登记的候选（来源：p4-deliver/sabh/REPORT.md；参照树 /content/sa_all/sa_plugin_sla @ 2cc12ea8）。候主会话确认。
