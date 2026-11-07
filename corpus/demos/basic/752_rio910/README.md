# 752 rio910

Riordan 续高位 `R(8)=91`，`R(9)=232`，`R(10)=603`（短递推续高位；644 体例续）。

- 对标：644_rio8 → `n=8` 版互补（同式再上两位；753 做续桥对峙）
- 绕行：`8·290/10=232`、`9·737/11=603` 先乘后除精确（596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/752_rio910/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/752_rio910/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/752_rio910/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/752_rio910/main.sla -o /tmp/basic_752 && /tmp/basic_752
# 期望输出：91,232,603
```
