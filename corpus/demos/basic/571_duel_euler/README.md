# 571 duel_euler

Eulerian 行和对峙：表行和 `==` 阶乘闭式（`n=4`：`24` 对 `24`；`n=5`：`120` 对 `120`）。

- 对标：570_euler_row → 单实现见源（286 系对峙体例；125 阶乘版互补）
- 绕行：`fact` 从 2 起乘（`fact(0/1)=1` 空积天然，记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/571_duel_euler/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/571_duel_euler/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/571_duel_euler/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/571_duel_euler/main.sla -o /tmp/basic_571 && /tmp/basic_571
# 期望输出：24,24,120,120
```
