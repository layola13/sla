# 581 duel_rowval

第一类行值对峙：表行和 `==` 阶乘闭式（`n=4`：`24` 对 `24`；`n=5`：`120` 对 `120`）。

- 对标：580_rowval → 单实现见源（286 系对峙体例；125 阶乘版互补）
- 绕行：`x=1` 代入行值即全排列计数（组合解释记 README，一语双关）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/581_duel_rowval/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/581_duel_rowval/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/581_duel_rowval/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/581_duel_rowval/main.sla -o /tmp/basic_581 && /tmp/basic_581
# 期望输出：24,24,120,120
```
