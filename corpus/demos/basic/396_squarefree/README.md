# 396 squarefree

无平方因子判定与计数（`<=20 → 13`，非者为 4/8/9/12/16/18/20；387 的存在版）。

- 对标：387 mobius → `d*d | n` 即判非（`12/18 → 0`）
- 绕行：无（纯 `int` 循环；`d*d` 整除判定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/396_squarefree/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/396_squarefree/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/396_squarefree/main.sla -o /tmp/basic_396 && /tmp/basic_396
# 期望输出：1,0,1,13
```
