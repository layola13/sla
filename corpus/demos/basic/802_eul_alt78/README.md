# 802 eul_alt78

Eulerian 交错续高位 `ealt(7,3)=2416`，`ealt(8,2)=4293`，`ealt(8,3)=15619`（交错纵深）。

- 对标：672_eul_alt → 低位交错版互补（同式再上两位；805 做三向收束）
- 绕行：`(8,3)` 五项 `65536-59049+9216-84=15619` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/802_eul_alt78/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/802_eul_alt78/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/802_eul_alt78/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/802_eul_alt78/main.sla -o /tmp/basic_802 && /tmp/basic_802
# 期望输出：2416,4293,15619
```
