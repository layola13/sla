# 530 compose

有序分拆（composition）：`c(n) = 2^(n-1)`（`c(1..5)=1,2,4,8,16`）。

- 对标：526_distinct → 无序版互补（有序每刀切分二选一，故为 2 幂）
- 绕行：纯 `*2` 循环（无 `**`/`<<` 算子，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/530_compose/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/530_compose/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/530_compose/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/530_compose/main.sla -o /tmp/basic_530 && /tmp/basic_530
# 期望输出：1,8,16
```
