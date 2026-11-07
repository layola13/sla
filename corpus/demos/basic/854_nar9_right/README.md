# 854 nar9_right

Narayana 第 9 行右部 `N(9,5)=1176`，`N(9,6)=336`，`N(9,7)=36`（846 左部版互补，全行覆盖）。

- 对标：846_nar9 → 左部版互补（左右合流全行 `1,36,336,1176,1764,1176,336,36,1`）
- 绕行：对称恒等 `N(9,6)=N(9,2)` 双算一致（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/854_nar9_right/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/854_nar9_right/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/854_nar9_right/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/854_nar9_right/main.sla -o /tmp/basic_854 && /tmp/basic_854
# 期望输出：1176,336,36
```
