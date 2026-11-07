# 796 eul7

Eulerian 续高位 `E(7,1)=120`，`E(7,2)=1191`，`E(7,3)=2416`（同式再上两行）。

- 对标：666_eulerian → `n≤6` 版互补（同式再上两行；799 做建表/交错和对峙）
- 绕行：`E(7,3)=4·302+4·302=2416` 对称双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/796_eul7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/796_eul7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/796_eul7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/796_eul7/main.sla -o /tmp/basic_796 && /tmp/basic_796
# 期望输出：120,1191,2416
```
