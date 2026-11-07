# FB-062 — `genImportedMacroArg` 的 bypass 后 `unreachable`

Zig `genImportedMacroArg` 的 bypass 后 `unreachable`——planner 契约一旦扩展出新 action，release 构建即 UB、debug 即 panic 且无诊断；建议 Zig 侧改响亮错误（同 SAB-D122）。

状态：四期 SAB-G 线登记的候选（来源：p4-deliver/sabg/REPORT.md；参照树 local-sa @ 3de91932，sab_codegen.zig 20,024 行版——与其余四期线所用 2cc12ea8 参照头不同，确认时须按现头复核行号）。Rust 侧按 SAB-D06 纪律移植为响亮错误（SAB-D122），候主会话确认。
