# 537 rr1

Rogers-Ramanujan 第一恒等式（小规模）：相邻部差 `≥2` 的分拆数 `==` 部皆 `≡1,4 (mod 5)` 的分拆数（`n=7`：`3` 对 `3`；`n=8`：`4` 对 `4`）。

- 对标：534_euler_duel → Euler 对峙版互补（此处为 RR 对峙；差分侧走双参递归）
- 绕行：递归深度 ≤8 小规模一次绿（07/120 递归先例）；`{1,4,6}` 三段直写（529 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/537_rr1/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/537_rr1/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/537_rr1/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/537_rr1/main.sla -o /tmp/basic_537 && /tmp/basic_537
# 期望输出：3,3,4,4
```
