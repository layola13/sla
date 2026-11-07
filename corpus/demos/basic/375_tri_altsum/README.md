# 375 tri altsum

交错和三向对峙（循环 vs 闭式 vs 逆序；368 的交错版）。

- 对标：368 tri_cubesum → 闭式 `偶 -n/2 / 奇 (n+1)/2` 锁定（`10 → -5` 三向）
- 绕行：无（`0 - n/2` 表负数，避一元负号解析坑；逆序有符号 `int`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/375_tri_altsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/375_tri_altsum/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/375_tri_altsum/main.sla -o /tmp/basic_375 && /tmp/basic_375
# 期望输出：-5,-5,-5
```
