# FB-064 — 宏实参绑定递归无环路守卫

`importedMacroValueArgNeedsRelease` 与 `importedMacroExistingAddressableSymbol` 经 arg binding 递归无环路守卫，自引用/环形宏实参绑定将无界递归（栈溢出、无诊断）；G 组移植原样保留递归结构，登记候裁。

状态：四期 SAB-G 线登记的候选（来源：p4-deliver/sabg/REPORT.md；参照树 local-sa @ 3de91932，sab_codegen.zig 20,024 行版，确认时须按现头复核）。候主会话确认。
