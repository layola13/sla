# 921 eul_alt9

Eulerian 交错续高位 `ealt(9,2)=14608`，`ealt(9,3)=88234`，`ealt(9,4)=156190`（交错纵深）。

- 对标：802_eul_alt78 → 低位交错版互补（交错纵深三值锁定；925 做三向收束）
- 绕行：`%2` 判奇偶交错求和（805 体例复用；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/921_eul_alt9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/921_eul_alt9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/921_eul_alt9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/921_eul_alt9/main.sla -o /tmp/basic_921 && /tmp/basic_921
# 期望输出：14608,88234,156190
```
