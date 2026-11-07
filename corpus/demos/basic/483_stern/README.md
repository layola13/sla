# 483 stern

Stern 双原子数列（`stern(10)=3`；237 的位折半版）。

- 对标：237 fib_table → 偶折半奇相加递归（`stern(9)=4`）
- 绕行：`n=0/1` 双基（整数除法天然折半；深度 log 级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/483_stern/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/483_stern/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/483_stern/main.sla -o /tmp/basic_483 && /tmp/basic_483
# 期望输出：3,4,3
```
