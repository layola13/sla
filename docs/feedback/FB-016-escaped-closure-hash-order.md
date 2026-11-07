# FB-016 — emitEscapedClosureEntries 以 AutoHashMap 迭代序决定发射序

- **反馈环批次**：第三批（SAB 二期 B 组移植中发现）
- **源登记**：`docs/divergences/sab.md` SAB-D18
- **分类**：Zig 侧缺陷候选 —— 源码机制定案（发射序非源码序、非插入序）
- **Rust 侧处置**：只镜像语义 + 登记。Rust 状态层把
  `escaped_closure_entries` 存为插入序 `Vec<(node_key, entry)>`
  （`crates/sla-codegen-sab/src/state.rs`），发射序由插入序决定；
  不复刻哈希序。

## 最小输入

任意含两个以上逃逸闭包（被 `spawn`/线程入口捕获环境）的程序；
闭包条数越多，哈希序与源码序分叉的概率越高。

## 机制（Zig 源码位置，本机参照树行号）

- `src/sab_codegen.zig` 字段（~330）：`escaped_closure_entries:
  std.AutoHashMap(*const ast.Node, EscapedClosureEntry)`——键是
  AST 节点指针，值是待发射的 worker/wrapper 条目。
- `emitEscapedClosureEntries`（~5900 附近）：`var iter =
  self.escaped_closure_entries.valueIterator(); while (iter.next())
  ...` 直接按哈希表迭代序逐条发射 spawn wrapper 与 worker 函数。
- 后果：同一程序的 SAB 字节中，逃逸闭包函数的排列顺序由指针哈希
  决定，不由源码顺序决定。指针地址随分配器/运行环境变化时，
  输出字节可随之变化（构建不可复现），且与 SA 文本路的源码序
  排列系统性不同，差分对拍时表现为「内容相同、顺序不同」。

## 建议修复形态（仅建议）

收集期同时维护一个插入序列表（或改用 `ArrayHashMap`），发射时
按插入序遍历；与主线既立的「发射路径禁止消费 HashMap 迭代序」
纪律对齐。修复后复核口径：多逃逸闭包程序在不同进程/分配器下
重复编译，SAB 字节逐字节恒同。

## Rust 侧锚点

`crates/sla-codegen-sab/src/state.rs::ProgramState::escaped_closure_entries`
（插入序 Vec，注释中登记了哈希序危害）。
