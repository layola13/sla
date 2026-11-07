# 442 lucas

Lucas 数（`L(10)=123`；237 的伴随版）。

- 对标：237 fib_table → 初值 `(2,1)` 迭代（`L(5)=11`）
- 绕行：`n=0/1` 提前返回（空循环初值坑，407 同类）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/442_lucas/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/442_lucas/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/442_lucas/main.sla -o /tmp/basic_442 && /tmp/basic_442
# 期望输出：123,11,2
```
