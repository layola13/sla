# FB-063 — `emitDirectImportedMacroCall` 对 arity 不符静默回退 fragment 拼接

`emitDirectImportedMacroCall` 对 arity 不符（如 ONCE_GET_OR_INIT 实参名数 ≠3）静默答 `emitted=false` 落回 fragment 拼接，失败点被推后且形态改变；建议核查该回退是否总能响亮失败而非误拼接。

状态：四期 SAB-G 线登记的候选（来源：p4-deliver/sabg/REPORT.md；参照树 local-sa @ 3de91932，sab_codegen.zig 20,024 行版，确认时须按现头复核）。候主会话确认后入 Zig 修复队列。
