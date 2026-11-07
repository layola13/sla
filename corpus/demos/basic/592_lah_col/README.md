# 592 lah_col

Lah 首列 `L(n,1)=n!`（`L(4,1)=24`，`L(5,1)=120`，`L(6,1)=720`）。

- 对标：586_lah → 全表版互补（此处首列单序列；582 首列体例，594 做对峙）
- 绕行：单块有序划分即全排列（组合解释记 README）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/592_lah_col/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/592_lah_col/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/592_lah_col/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/592_lah_col/main.sla -o /tmp/basic_592 && /tmp/basic_592
# 期望输出：24,120,720
```
