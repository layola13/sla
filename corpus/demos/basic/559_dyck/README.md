# 559 dyck

Dyck 路经数（网格 DP，不越过对角线）：`D(n,n)=C(n)`（`5,14,42`）。

- 对标：531_stirling2 → 同为拍扁二维表（此处为正方形网格带 `b≤a` 对角守卫）
- 绕行：嵌套 `while` 内层不加 `;`（386 同例）；零表天然处理越界区（`b>a` 恒零）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/559_dyck/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/559_dyck/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/559_dyck/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/559_dyck/main.sla -o /tmp/basic_559 && /tmp/basic_559
# 期望输出：5,14,42
```
