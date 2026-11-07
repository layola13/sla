# 420 cunningham

Cunningham 链长（一类；419 的链版）。

- 对标：419 sophie → `2p+1` 迭代至合数（`2 → 5` 项经 47，`5 → 4` 项）
- 绕行：无（函数调用作 `while` 条件；纯 `int` 迭代）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/420_cunningham/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/420_cunningham/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/420_cunningham/main.sla -o /tmp/basic_420 && /tmp/basic_420
# 期望输出：5,2,4
```
